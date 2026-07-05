//! Tree-walking interpreter: runs an [`App`] and captures its printed output.
//! Supports functions, classes (methods + `self`), exceptions (`try`/`catch`/
//! `throw`), `break`/`continue`, and eager generators (`yield`).
//!
//! Performance model: the program's functions and classes are compiled once
//! into an [`Arc`]-shared [`Program`] (function bodies are never cloned per
//! call — calls are an `Arc` bump). Scopes are fast-hashed maps, `for` loops
//! iterate shared lists without copying them, and the accumulate patterns
//! (`xs = push(xs, v)`, `m[k] = v`) mutate in place when the collection is
//! uniquely owned instead of rebuilding it — turning the naive O(n²) shapes an
//! LLM naturally writes into O(n).

use crate::ast::*;
use crate::builtins;
use crate::fxhash::FxHashMap;
use crate::value::{StreamHandle, StreamMsg, Value};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A variable scope: name → value, fast-hashed (names are short and trusted).
pub(crate) type Scope = FxHashMap<String, Value>;

/// Hard resource constraints for a run. Any breach aborts the offending
/// statement with a recoverable [`Signal::Error`] *before* the process can
/// exhaust memory or hang — this is the in-process half of the OOM/timeout
/// protection (the out-of-process half lives in `nexus-runtime`).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Max interpreter steps (guards infinite loops / runaway recursion).
    pub steps: u64,
    /// Max cumulative collection cells allocated (a memory-pressure proxy).
    pub alloc_cells: u64,
    /// Optional wall-clock budget.
    pub wall: Option<Duration>,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { steps: 50_000_000, alloc_cells: 1_000_000_000, wall: None }
    }
}

/// Outcome of a guarded run: whatever output was produced, plus whether a
/// breach/error was caught and the state rolled back to the last consistent
/// checkpoint.
#[derive(Debug)]
pub struct GuardedOutcome {
    pub output: Vec<String>,
    pub error: Option<String>,
    pub recovered: bool,
    pub steps: u64,
    pub alloc_cells: u64,
    /// Live variables after rollback (the recovered, consistent state).
    pub live_vars: usize,
}

/// Non-local control flow that unwinds the stack: an error or a thrown value.
/// Modeling both uniformly lets `try`/`catch` catch runtime errors and explicit
/// `throw`s alike.
#[derive(Debug)]
pub enum Signal {
    Error(String),
    Throw(Value),
}

impl From<String> for Signal {
    fn from(s: String) -> Self {
        Signal::Error(s)
    }
}

impl Signal {
    fn message(self) -> String {
        match self {
            Signal::Error(s) => s,
            Signal::Throw(v) => format!("uncaught exception: {}", v),
        }
    }
}

type ER<T> = Result<T, Signal>;

/// Statement-level control flow within a function body.
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

/// Host-provided builtins (e.g. the declarative context).
pub trait HostFns {
    fn call(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>>;
}

/// The immutable, shareable half of a running program: functions and classes,
/// indexed for O(1) lookup with `Arc`-shared bodies. Built once by
/// [`Interp::new`]; every call site and worker thread shares it by refcount.
pub(crate) struct Program {
    funcs: FxHashMap<String, Arc<Func>>,
    classes: FxHashMap<String, Arc<ClassRT>>,
}

/// A class compiled for dispatch: methods by name instead of a linear list.
pub(crate) struct ClassRT {
    methods: FxHashMap<String, Arc<Func>>,
}

impl Program {
    fn compile(app: &App) -> Program {
        let mut funcs = FxHashMap::default();
        for f in &app.funcs {
            funcs.insert(f.name.clone(), Arc::new(f.clone()));
        }
        let mut classes = FxHashMap::default();
        for c in &app.classes {
            let mut methods = FxHashMap::default();
            for m in &c.methods {
                methods.insert(m.name.clone(), Arc::new(m.clone()));
            }
            classes.insert(c.name.clone(), Arc::new(ClassRT { methods }));
        }
        Program { funcs, classes }
    }
}

/// A running interpreter.
///
/// The whole program lives behind an [`Arc`] so a parallel builtin (`pmap`,
/// `parallel`) can cheaply share it with worker threads, each of which spins up
/// its own throwaway [`Interp`] over the same definitions.
pub struct Interp {
    prog: Arc<Program>,
    pub output: Vec<String>,
    steps: u64,
    step_limit: u64,
    /// Cumulative collection cells allocated (memory-pressure accounting).
    alloc: u64,
    alloc_limit: u64,
    /// Set when a wall-clock budget is in force.
    started: Option<Instant>,
    wall_limit: Option<Duration>,
    host: Option<Box<dyn HostFns>>,
    /// When this interpreter is running a generator body on a worker thread,
    /// `yield` sends items here (a bounded channel → backpressure → streaming).
    yield_sink: Option<SyncSender<StreamMsg>>,
    /// Memoized purity of user functions, for transparent auto-parallelization.
    pure_cache: FxHashMap<String, bool>,
    /// Current user-function call nesting. Guards against unbounded recursion
    /// overflowing the native stack (an uncatchable abort) before the step or
    /// wall-clock budget can trip. See [`MAX_CALL_DEPTH`].
    call_depth: u32,
}

/// Ceiling on user-function call nesting. A recursive `.nx` program deeper than
/// this gets a catchable runtime error instead of a stack-overflow abort.
///
/// The bound must be safe on the *smallest* stack any interpreter runs on: the
/// main CLI path uses a 64 MiB worker thread, but generator bodies and the
/// parallel builtins (`pmap`/`parallel`) spawn worker threads with the default
/// stack (~1-2 MiB), and library callers of `run_source` run on the caller's
/// thread. Each interpreter frame is heavy (a full `eval`/`exec_block` chain),
/// so we keep this conservative. 128 matches the parser's `MAX_EXPR_DEPTH`
/// (already proven safe on the default stack) and is deeper than any realistic
/// call nesting while staying clear of a 1 MiB stack.
const MAX_CALL_DEPTH: u32 = 128;

impl Interp {
    pub fn new(app: &App) -> Interp {
        Interp::from_parts(Arc::new(Program::compile(app)))
    }

    /// Build an interpreter over an already-shared program (used by worker
    /// threads in the parallel builtins).
    fn from_parts(prog: Arc<Program>) -> Interp {
        let l = Limits::default();
        Interp {
            prog,
            output: Vec::new(),
            steps: 0,
            step_limit: l.steps,
            alloc: 0,
            alloc_limit: l.alloc_cells,
            started: None,
            wall_limit: None,
            host: None,
            yield_sink: None,
            pure_cache: FxHashMap::default(),
            call_depth: 0,
        }
    }

    /// Apply hard resource limits and arm the wall-clock if set.
    pub fn set_limits(&mut self, limits: Limits) {
        self.step_limit = limits.steps;
        self.alloc_limit = limits.alloc_cells;
        self.wall_limit = limits.wall;
        if limits.wall.is_some() {
            self.started = Some(Instant::now());
        }
    }

    /// Charge `n` allocated cells against the memory budget.
    fn note_alloc(&mut self, n: usize) -> ER<()> {
        self.alloc = self.alloc.saturating_add(n as u64);
        if self.alloc > self.alloc_limit {
            return Err(Signal::Error(format!(
                "allocation limit exceeded ({} > {} cells)",
                self.alloc, self.alloc_limit
            )));
        }
        Ok(())
    }

    pub fn run(app: &App) -> Result<Vec<String>, String> {
        let mut it = Interp::new(app);
        let mut scope = Scope::default();
        it.exec_main(&app.main, &mut scope).map_err(Signal::message)?;
        Ok(it.output)
    }

    pub fn run_with_host(app: &App, host: Box<dyn HostFns>) -> Result<Vec<String>, String> {
        let mut it = Interp::new(app);
        it.host = Some(host);
        let mut scope = Scope::default();
        it.exec_main(&app.main, &mut scope).map_err(Signal::message)?;
        Ok(it.output)
    }

    /// Execute the top-level statement block, rejecting loop-control flow that
    /// escaped every loop.
    fn exec_main(&mut self, stmts: &[Stmt], scope: &mut Scope) -> ER<()> {
        match self.exec_block(stmts, scope)? {
            Flow::Break => Err(Signal::Error("`break` outside a loop".into())),
            Flow::Continue => Err(Signal::Error("`continue` outside a loop".into())),
            _ => Ok(()),
        }
    }

    /// Run `main` under hard resource limits with transactional rollback.
    ///
    /// Each top-level statement is a checkpoint: it executes against a snapshot
    /// of the variable scope, and if it breaches a limit or errors, the scope is
    /// rolled back to the last consistent checkpoint and the run stops cleanly.
    /// Output produced before the failure is retained, so a long automation can
    /// fail one step and still hand back everything that already succeeded —
    /// recovery rather than a hard crash.
    pub fn run_guarded(app: &App, limits: Limits) -> GuardedOutcome {
        let mut it = Interp::new(app);
        it.set_limits(limits);
        let mut scope = Scope::default();
        let mut last_good = scope.clone();
        let mut error = None;
        let mut recovered = false;
        for stmt in &app.main {
            match it.exec_stmt(stmt, &mut scope) {
                Ok(Flow::Normal) => last_good = scope.clone(),
                Ok(Flow::Return(_)) => break,
                Ok(Flow::Break) | Ok(Flow::Continue) => {
                    error = Some("`break`/`continue` outside a loop".into());
                    scope = last_good.clone();
                    recovered = true;
                    break;
                }
                Err(sig) => {
                    error = Some(sig.message());
                    scope = last_good.clone(); // rollback to last consistent state
                    recovered = true;
                    break;
                }
            }
        }
        GuardedOutcome {
            output: it.output,
            error,
            recovered,
            steps: it.steps,
            alloc_cells: it.alloc,
            live_vars: scope.len(),
        }
    }

    fn tick(&mut self) -> ER<()> {
        self.steps += 1;
        if self.steps > self.step_limit {
            return Err(Signal::Error("execution step limit exceeded (possible infinite loop)".into()));
        }
        // Wall-clock budget, checked sparsely to keep the hot path cheap.
        if self.steps & 0xFFF == 0 {
            if let (Some(start), Some(limit)) = (self.started, self.wall_limit) {
                if start.elapsed() > limit {
                    return Err(Signal::Error("wall-clock time limit exceeded".into()));
                }
            }
        }
        Ok(())
    }

    fn exec_block(&mut self, stmts: &[Stmt], scope: &mut Scope) -> ER<Flow> {
        for s in stmts {
            match self.exec_stmt(s, scope)? {
                Flow::Normal => {}
                ret => return Ok(ret),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_stmt(&mut self, s: &Stmt, scope: &mut Scope) -> ER<Flow> {
        self.tick()?;
        match s {
            Stmt::Assign(name, e) => {
                // Accumulate fast path: `xs = push(xs, v)` / `m = set(m, k, v)`
                // reuse the existing collection in place when uniquely owned,
                // instead of copying the whole thing (O(n²) → O(n)).
                if let Some(flow) = self.try_accumulate(name, e, scope)? {
                    return Ok(flow);
                }
                let v = self.eval(e, scope)?;
                assign(scope, name, v);
                Ok(Flow::Normal)
            }
            Stmt::IndexAssign(name, idx_exprs, e) => {
                let v = self.eval(e, scope)?;
                let mut indices = Vec::with_capacity(idx_exprs.len());
                for ix in idx_exprs {
                    indices.push(self.eval(ix, scope)?);
                }
                // Take the current value out of its slot so the mutation sees a
                // uniquely-owned collection (no copy) in the common case.
                let mut cur = match scope.get_mut(name) {
                    Some(slot) => std::mem::replace(slot, Value::Nil),
                    None => Value::Nil,
                };
                let res = builtins::set_path_mut(&mut cur, &indices, v);
                assign(scope, name, cur);
                res.map_err(Signal::from)?;
                Ok(Flow::Normal)
            }
            Stmt::Print(e) => {
                let v = self.eval(e, scope)?;
                self.output.push(v.to_string());
                Ok(Flow::Normal)
            }
            Stmt::Return(e) => {
                let v = self.eval(e, scope)?;
                Ok(Flow::Return(v))
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Throw(e) => {
                let v = self.eval(e, scope)?;
                Err(Signal::Throw(v))
            }
            Stmt::Yield(e) => {
                let v = self.eval(e, scope)?;
                match &self.yield_sink {
                    // Blocks when the channel is full → the generator only runs
                    // as fast as the consumer pulls (true streaming). A send
                    // error means the consumer stopped early; unwind cleanly.
                    Some(tx) => tx
                        .send(StreamMsg::Item(v))
                        .map_err(|_| Signal::Error("stream consumer stopped".into()))?,
                    None => return Err(Signal::Error("`yield` outside a generator function".into())),
                }
                Ok(Flow::Normal)
            }
            Stmt::Try(body, var, handler) => match self.exec_block(body, scope) {
                Ok(flow) => Ok(flow),
                Err(sig) => {
                    let exc = match sig {
                        Signal::Throw(v) => v,
                        Signal::Error(msg) => Value::str(msg),
                    };
                    assign(scope, var, exc);
                    self.exec_block(handler, scope)
                }
            },
            Stmt::If(cond, then, els) => {
                if self.eval(cond, scope)?.truthy() {
                    self.exec_block(then, scope)
                } else {
                    self.exec_block(els, scope)
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond, scope)?.truthy() {
                    self.tick()?;
                    match self.exec_block(body, scope)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        ret @ Flow::Return(_) => return Ok(ret),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For(var, iter, body) => {
                let seq = self.eval(iter, scope)?;
                match &seq {
                    // Streams are pulled lazily — one item at a time, so an
                    // infinite generator can drive a `for` loop with flat memory.
                    Value::Stream(s) => {
                        let s = Arc::clone(s);
                        while let Some(item) = s.next() {
                            let item = item.map_err(Signal::Error)?;
                            self.tick()?;
                            assign(scope, var, item);
                            match self.exec_block(body, scope)? {
                                Flow::Normal | Flow::Continue => {}
                                Flow::Break => break,
                                ret @ Flow::Return(_) => return Ok(ret),
                            }
                        }
                        Ok(Flow::Normal)
                    }
                    // Lists iterate over the shared backing store directly —
                    // no up-front copy of the whole collection.
                    Value::List(l) => {
                        let l = Arc::clone(l);
                        for item in l.iter() {
                            self.tick()?;
                            assign(scope, var, item.clone());
                            match self.exec_block(body, scope)? {
                                Flow::Normal | Flow::Continue => {}
                                Flow::Break => break,
                                ret @ Flow::Return(_) => return Ok(ret),
                            }
                        }
                        Ok(Flow::Normal)
                    }
                    _ => {
                        let items = builtins::iterate(&seq)?;
                        for item in items {
                            self.tick()?;
                            assign(scope, var, item);
                            match self.exec_block(body, scope)? {
                                Flow::Normal | Flow::Continue => {}
                                Flow::Break => break,
                                ret @ Flow::Return(_) => return Ok(ret),
                            }
                        }
                        Ok(Flow::Normal)
                    }
                }
            }
            Stmt::Expr(e) => {
                self.eval(e, scope)?;
                Ok(Flow::Normal)
            }
        }
    }

    /// Detect and execute the self-accumulate pattern `x = push(x, v)` /
    /// `x = set(x, k, v)` by mutating `x` in place when uniquely owned.
    ///
    /// `push`/`set` are core builtins: user functions with those names take
    /// precedence (checked below), but host contexts cannot override them.
    fn try_accumulate(&mut self, name: &str, e: &AExpr, scope: &mut Scope) -> ER<Option<Flow>> {
        let (fname, args) = match e {
            AExpr::Call(f, a) => (f.as_str(), a.as_slice()),
            _ => return Ok(None),
        };
        let is_push = fname == "push" && args.len() == 2;
        let is_set = fname == "set" && args.len() == 3;
        if !(is_push || is_set) || self.prog.funcs.contains_key(fname) {
            return Ok(None);
        }
        if !matches!(&args[0], AExpr::Var(v) if v == name) || !scope.contains_key(name) {
            return Ok(None);
        }
        // Evaluate the remaining arguments BEFORE detaching the collection, so
        // expressions that read the same variable (e.g. `push(xs, len(xs))`)
        // still see it.
        let mut argv = Vec::with_capacity(args.len() - 1);
        for a in &args[1..] {
            argv.push(self.eval(a, scope)?);
        }
        let slot = scope.get_mut(name).expect("checked above");
        let mut cur = std::mem::replace(slot, Value::Nil);
        let res = if is_push {
            builtins::push_mut(&mut cur, argv.pop().expect("push arg"))
        } else {
            let v = argv.pop().expect("set value");
            let k = argv.pop().expect("set key");
            builtins::index_set_mut(&mut cur, &k, v)
        };
        self.note_alloc(1)?;
        assign(scope, name, cur);
        res.map_err(Signal::from)?;
        Ok(Some(Flow::Normal))
    }

    fn eval(&mut self, e: &AExpr, scope: &mut Scope) -> ER<Value> {
        self.tick()?;
        match e {
            AExpr::Num(n) => Ok(Value::Num(*n)),
            AExpr::Str(s) => Ok(Value::str(s.clone())),
            AExpr::Bool(b) => Ok(Value::Bool(*b)),
            AExpr::Nil => Ok(Value::Nil),
            AExpr::Var(name) => scope
                .get(name)
                .cloned()
                .ok_or_else(|| Signal::Error(format!("undefined variable `{}`", name))),
            AExpr::List(items) => {
                let mut v = Vec::with_capacity(items.len());
                for it in items {
                    v.push(self.eval(it, scope)?);
                }
                self.note_alloc(v.len())?;
                Ok(Value::list(v))
            }
            AExpr::MapLit(pairs) => {
                let mut m = std::collections::BTreeMap::new();
                for (k, val) in pairs {
                    let key = self.eval(k, scope)?;
                    let key = key
                        .as_str()
                        .map_err(|_| Signal::Error("map literal keys must be strings".into()))?
                        .to_string();
                    let v = self.eval(val, scope)?;
                    m.insert(key, v);
                }
                self.note_alloc(m.len())?;
                Ok(Value::map(m))
            }
            AExpr::Interp(parts) => {
                let mut s = String::new();
                for p in parts {
                    match p {
                        IPart::Lit(text) => s.push_str(text),
                        IPart::Expr(e) => {
                            use std::fmt::Write;
                            let v = self.eval(e, scope)?;
                            let _ = write!(s, "{}", v);
                        }
                    }
                }
                Ok(Value::str(s))
            }
            AExpr::Comp { elem, var, iter, cond } => {
                let seq = self.eval(iter, scope)?;
                let items = builtins::iter_vals(&seq)?;
                // Transparent auto-parallelization: a large comprehension whose
                // body is side-effect-free and does real per-element work
                // (calls a user function) is fanned across threads — same
                // result, just faster — without the author asking. Anything
                // unprovable stays serial, so correctness is never at risk.
                if self.should_auto_parallelize(elem, cond, items.len()) {
                    nexus_core::trace::record(
                        nexus_core::trace::Level::Debug,
                        "exec.autopar",
                        None,
                        "comprehension",
                        &[("items", &items.len().to_string())],
                    );
                    let out = par_eval_comp(
                        Arc::clone(&self.prog),
                        scope.clone(),
                        var.clone(),
                        (**elem).clone(),
                        cond.clone(),
                        items.into_owned(),
                    )?;
                    self.note_alloc(out.len())?;
                    return Ok(Value::list(out));
                }
                let mut out = Vec::new();
                for item in items.iter() {
                    assign(scope, var, item.clone());
                    if let Some(c) = cond {
                        if !self.eval(c, scope)?.truthy() {
                            continue;
                        }
                    }
                    out.push(self.eval(elem, scope)?);
                    self.note_alloc(1)?;
                }
                Ok(Value::list(out))
            }
            AExpr::Unary(op, a) => {
                let av = self.eval(a, scope)?;
                match op {
                    UnOp::Neg => Ok(Value::Num(-av.as_num()?)),
                    UnOp::Not => Ok(Value::Bool(!av.truthy())),
                }
            }
            AExpr::Binary(op, a, b) => self.eval_binary(*op, a, b, scope),
            AExpr::Index(coll, idx) => {
                let c = self.eval(coll, scope)?;
                let i = self.eval(idx, scope)?;
                Ok(builtins::index_get(&c, &i)?)
            }
            AExpr::Call(name, args) => {
                let mut argv = Vec::with_capacity(args.len());
                for a in args {
                    argv.push(self.eval(a, scope)?);
                }
                self.call(name, argv)
            }
            AExpr::Method(recv, name, args) => {
                let recv_val = self.eval(recv, scope)?;
                let mut argv = Vec::with_capacity(args.len());
                for a in args {
                    argv.push(self.eval(a, scope)?);
                }
                self.dispatch_method(recv_val, name, argv)
            }
        }
    }

    fn eval_binary(&mut self, op: BinOp, a: &AExpr, b: &AExpr, scope: &mut Scope) -> ER<Value> {
        match op {
            BinOp::And => {
                let av = self.eval(a, scope)?;
                return if av.truthy() { self.eval(b, scope) } else { Ok(av) };
            }
            BinOp::Or => {
                let av = self.eval(a, scope)?;
                return if av.truthy() { Ok(av) } else { self.eval(b, scope) };
            }
            _ => {}
        }
        let av = self.eval(a, scope)?;
        let bv = self.eval(b, scope)?;
        Ok(builtins::binary(op, &av, &bv)?)
    }

    /// Run a user function (or method) with a prepared local scope, handling
    /// eager generator collection.
    fn run_func(&mut self, f: &Arc<Func>, local: Scope) -> ER<Value> {
        if f.is_generator {
            return Ok(self.spawn_generator(f, local));
        }
        // Bound recursion so a runaway self-call returns a catchable error
        // instead of overflowing the native stack (which would abort the whole
        // process, uncatchable even under `run-guarded`).
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Signal::Error(format!(
                "maximum call depth exceeded ({MAX_CALL_DEPTH}); possible unbounded recursion"
            )));
        }
        self.call_depth += 1;
        let mut local = local;
        let result = self.exec_block(&f.body, &mut local);
        self.call_depth -= 1;
        match result? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Value::Nil),
            Flow::Break => Err(Signal::Error("`break` outside a loop".into())),
            Flow::Continue => Err(Signal::Error("`continue` outside a loop".into())),
        }
    }

    /// Launch a generator body on a worker thread and hand back a lazy
    /// [`Value::Stream`]. The thread produces items into a bounded channel, so
    /// it runs only as far ahead as the consumer pulls — memory stays flat and
    /// infinite generators are fine (consume with `take`).
    fn spawn_generator(&self, f: &Arc<Func>, local: Scope) -> Value {
        use std::sync::mpsc::sync_channel;
        let (tx, rx) = sync_channel::<StreamMsg>(64);
        let prog = Arc::clone(&self.prog);
        let f = Arc::clone(f);
        std::thread::spawn(move || {
            let mut w = Interp::from_parts(prog);
            w.yield_sink = Some(tx.clone());
            let mut scope = local;
            if let Err(sig) = w.exec_block(&f.body, &mut scope) {
                // Forward a genuine error; the "consumer stopped" sentinel just
                // means the receiver was dropped, so this send is a harmless no-op.
                let _ = tx.send(StreamMsg::Err(sig.message()));
            }
        });
        Value::Stream(Arc::new(StreamHandle::new(rx)))
    }

    fn call(&mut self, name: &str, args: Vec<Value>) -> ER<Value> {
        // Language-level parallelism and higher-order builtins: these invoke
        // user functions, so they live here rather than in `builtins`.
        match name {
            "pmap" => return self.par_map(args),
            "amap" => return self.auto_map(args),
            "parallel" => return self.par_run(args),
            "filter" => return self.filter_hof(args),
            "reduce" => return self.reduce_hof(args),
            _ => {}
        }
        // Constructor: `ClassName(args)` builds an instance.
        if self.prog.classes.contains_key(name) {
            return self.construct(name, args);
        }
        // User-defined function.
        if let Some(f) = self.prog.funcs.get(name) {
            let f = Arc::clone(f);
            check_arity(name, &f.params, args.len())?;
            let mut local = Scope::default();
            for (p, v) in f.params.iter().zip(args) {
                local.insert(p.clone(), v);
            }
            return self.run_func(&f, local);
        }
        // Host-provided builtin (declarative context).
        if let Some(host) = &self.host {
            if let Some(result) = host.call(name, &args) {
                return result.map_err(Signal::from);
            }
        }
        // Standard builtin.
        let r = builtins::call(name, &args).map_err(Signal::from)?;
        self.note_alloc(cells(&r))?;
        Ok(r)
    }

    /// `recv.method(args)`: dispatch to a class method if `recv` is an instance,
    /// otherwise treat as `method(recv, args)` (string/list/map builtins).
    fn dispatch_method(&mut self, recv: Value, name: &str, args: Vec<Value>) -> ER<Value> {
        if let Value::Map(m) = &recv {
            if let Some(Value::Str(cls)) = m.get("__class__") {
                if let Some(class) = self.prog.classes.get(cls.as_str()) {
                    if let Some(method) = class.methods.get(name) {
                        let method = Arc::clone(method);
                        let mut local = Scope::default();
                        local.insert("self".into(), recv.clone());
                        for (p, v) in method.params.iter().skip(1).zip(args) {
                            local.insert(p.clone(), v);
                        }
                        return self.run_func(&method, local);
                    }
                }
            }
        }
        // Fall back to a free function / builtin with recv as first argument.
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(recv);
        all.extend(args);
        self.call(name, all)
    }

    /// `filter("pred", seq)` — keep the elements where the named user function
    /// returns a truthy value.
    fn filter_hof(&mut self, args: Vec<Value>) -> ER<Value> {
        if args.len() != 2 {
            return Err(Signal::Error("filter expects (fn_name, list)".into()));
        }
        let fname = args[0].as_str().map_err(Signal::from)?.to_string();
        let items = builtins::iterate(&args[1]).map_err(Signal::from)?;
        let mut out = Vec::new();
        for it in items {
            if self.call(&fname, vec![it.clone()])?.truthy() {
                out.push(it);
                self.note_alloc(1)?;
            }
        }
        Ok(Value::list(out))
    }

    /// `reduce("fn", seq, init)` — left fold: `acc = fn(acc, item)`.
    fn reduce_hof(&mut self, args: Vec<Value>) -> ER<Value> {
        if args.len() != 3 {
            return Err(Signal::Error("reduce expects (fn_name, list, init)".into()));
        }
        let fname = args[0].as_str().map_err(Signal::from)?.to_string();
        let items = builtins::iterate(&args[1]).map_err(Signal::from)?;
        let mut acc = args[2].clone();
        for it in items {
            acc = self.call(&fname, vec![acc, it])?;
        }
        Ok(acc)
    }

    /// `pmap(fn_name, list)` — apply a 1-arg user function to every element of
    /// `list` across worker threads, returning results in input order.
    fn par_map(&mut self, args: Vec<Value>) -> ER<Value> {
        if args.len() != 2 {
            return Err(Signal::Error("pmap expects (fn_name, list)".into()));
        }
        let fname = args[0].as_str().map_err(Signal::from)?.to_string();
        if !self.prog.funcs.contains_key(&fname) {
            return Err(Signal::Error(format!("pmap: unknown function `{}`", fname)));
        }
        let items = builtins::iterate(&args[1]).map_err(Signal::from)?;
        let jobs: Vec<(String, Vec<Value>)> = items.into_iter().map(|it| (fname.clone(), vec![it])).collect();
        let (vals, outs) = par_calls(Arc::clone(&self.prog), jobs)?;
        self.output.extend(outs);
        Ok(Value::list(vals))
    }

    /// `amap(fn_name, list)` — *automatic* multithreading: the runtime decides
    /// whether to parallelize. Small inputs run inline (no thread overhead);
    /// large inputs fan across cores. Same result either way, so callers never
    /// have to reason about when threading pays off.
    fn auto_map(&mut self, args: Vec<Value>) -> ER<Value> {
        if args.len() != 2 {
            return Err(Signal::Error("amap expects (fn_name, list)".into()));
        }
        let fname = args[0].as_str().map_err(Signal::from)?.to_string();
        if !self.prog.funcs.contains_key(&fname) {
            return Err(Signal::Error(format!("amap: unknown function `{}`", fname)));
        }
        let items = builtins::iterate(&args[1]).map_err(Signal::from)?;
        // Below the threshold (or with only one core) the serial path wins.
        const PAR_THRESHOLD: usize = 64;
        if items.len() < PAR_THRESHOLD || nexus_accel::cores() <= 1 {
            nexus_core::trace::record(
                nexus_core::trace::Level::Debug,
                "exec.amap",
                None,
                "serial",
                &[("items", &items.len().to_string())],
            );
            let f = Arc::clone(self.prog.funcs.get(&fname).expect("checked above"));
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let mut local = Scope::default();
                if let Some(p) = f.params.first() {
                    local.insert(p.clone(), it);
                }
                out.push(self.run_func(&f, local)?);
            }
            return Ok(Value::list(out));
        }
        nexus_core::trace::record(
            nexus_core::trace::Level::Debug,
            "exec.amap",
            None,
            "parallel",
            &[("items", &items.len().to_string())],
        );
        let jobs: Vec<(String, Vec<Value>)> = items.into_iter().map(|it| (fname.clone(), vec![it])).collect();
        let (vals, outs) = par_calls(Arc::clone(&self.prog), jobs)?;
        self.output.extend(outs);
        Ok(Value::list(vals))
    }

    /// Decide whether a comprehension should be transparently parallelized.
    /// Conservative: only when it is large, the machine has cores to spare,
    /// auto-par isn't disabled, the body does real work (calls a user
    /// function), and the body is *provably* side-effect free. Host-provided
    /// builtins (e.g. `validate`) can't cross threads, so the purity analysis
    /// rejects any name that isn't a user function or a known-pure core
    /// builtin — which is what makes this safe even in mixed programs.
    fn should_auto_parallelize(&mut self, elem: &AExpr, cond: &Option<Box<AExpr>>, n: usize) -> bool {
        const THRESHOLD: usize = 256;
        if n < THRESHOLD || nexus_accel::cores() <= 1 || !auto_par_enabled() {
            return false;
        }
        if !self.body_does_work(elem) {
            return false;
        }
        let mut visiting = std::collections::HashSet::new();
        if !self.expr_pure(elem, &mut visiting) {
            return false;
        }
        if let Some(c) = cond {
            if !self.expr_pure(c, &mut visiting) {
                return false;
            }
        }
        true
    }

    /// True if the expression calls a user function (a sign that per-element
    /// work is substantial enough for threading to pay off).
    fn body_does_work(&self, e: &AExpr) -> bool {
        match e {
            AExpr::Call(name, args) => {
                self.prog.funcs.contains_key(name) || args.iter().any(|a| self.body_does_work(a))
            }
            AExpr::Method(recv, _, args) => {
                // A method call may be a class method (real work).
                !self.prog.classes.is_empty()
                    || self.body_does_work(recv)
                    || args.iter().any(|a| self.body_does_work(a))
            }
            AExpr::Binary(_, a, b) | AExpr::Index(a, b) => self.body_does_work(a) || self.body_does_work(b),
            AExpr::Unary(_, a) => self.body_does_work(a),
            AExpr::List(items) => items.iter().any(|i| self.body_does_work(i)),
            AExpr::MapLit(pairs) => pairs.iter().any(|(k, v)| self.body_does_work(k) || self.body_does_work(v)),
            AExpr::Comp { elem, iter, cond, .. } => {
                self.body_does_work(elem)
                    || self.body_does_work(iter)
                    || cond.as_ref().is_some_and(|c| self.body_does_work(c))
            }
            AExpr::Interp(parts) => parts.iter().any(|p| matches!(p, IPart::Expr(e) if self.body_does_work(e))),
            _ => false,
        }
    }

    /// Is this expression provably free of side effects (no print/yield/IO)?
    fn expr_pure(&mut self, e: &AExpr, visiting: &mut std::collections::HashSet<String>) -> bool {
        match e {
            AExpr::Num(_) | AExpr::Str(_) | AExpr::Bool(_) | AExpr::Nil | AExpr::Var(_) => true,
            AExpr::List(items) => items.iter().all(|i| self.expr_pure(i, visiting)),
            AExpr::MapLit(pairs) => pairs.iter().all(|(k, v)| self.expr_pure(k, visiting) && self.expr_pure(v, visiting)),
            AExpr::Interp(parts) => parts.iter().all(|p| match p {
                IPart::Lit(_) => true,
                IPart::Expr(e) => self.expr_pure(e, visiting),
            }),
            AExpr::Comp { elem, iter, cond, .. } => {
                self.expr_pure(elem, visiting)
                    && self.expr_pure(iter, visiting)
                    && cond.as_ref().is_none_or(|c| self.expr_pure(c, visiting))
            }
            AExpr::Unary(_, a) => self.expr_pure(a, visiting),
            AExpr::Binary(_, a, b) | AExpr::Index(a, b) => self.expr_pure(a, visiting) && self.expr_pure(b, visiting),
            AExpr::Call(name, args) => args.iter().all(|a| self.expr_pure(a, visiting)) && self.callee_pure(name, visiting),
            AExpr::Method(recv, name, args) => {
                self.expr_pure(recv, visiting)
                    && args.iter().all(|a| self.expr_pure(a, visiting))
                    && self.method_pure(name, visiting)
            }
        }
    }

    /// Purity of a called name: impure builtins fail; user functions/constructors
    /// are analyzed; core pure builtins pass. Anything else (host-provided
    /// builtins, typos) is treated as impure so it never crosses a thread.
    fn callee_pure(&mut self, name: &str, visiting: &mut std::collections::HashSet<String>) -> bool {
        if IMPURE_BUILTINS.contains(&name) {
            return false;
        }
        if self.prog.classes.contains_key(name) {
            return self.class_init_pure(name, visiting);
        }
        if self.prog.funcs.contains_key(name) {
            return self.func_is_pure(name, visiting);
        }
        builtins::PURE_BUILTINS.contains(&name)
    }

    /// Purity of a method name: impure builtins fail; a class method with this
    /// name must be pure; otherwise it must be a known-pure builtin.
    fn method_pure(&mut self, name: &str, visiting: &mut std::collections::HashSet<String>) -> bool {
        if IMPURE_BUILTINS.contains(&name) {
            return false;
        }
        let is_class_method = self.prog.classes.values().any(|c| c.methods.contains_key(name));
        if !is_class_method && !builtins::PURE_BUILTINS.contains(&name) {
            return false;
        }
        let methods: Vec<Arc<Func>> = self
            .prog
            .classes
            .values()
            .filter_map(|c| c.methods.get(name).cloned())
            .collect();
        methods
            .iter()
            .all(|m| self.func_body_pure(&format!("method:{}", name), &m.body, visiting))
    }

    fn class_init_pure(&mut self, class: &str, visiting: &mut std::collections::HashSet<String>) -> bool {
        let init = self.prog.classes.get(class).and_then(|c| c.methods.get("init").cloned());
        match init {
            Some(f) => self.func_body_pure(&format!("init:{}", class), &f.body, visiting),
            None => true,
        }
    }

    fn func_is_pure(&mut self, name: &str, visiting: &mut std::collections::HashSet<String>) -> bool {
        if let Some(&p) = self.pure_cache.get(name) {
            return p;
        }
        let f = match self.prog.funcs.get(name) {
            Some(f) => Arc::clone(f),
            None => return false,
        };
        if f.is_generator {
            self.pure_cache.insert(name.to_string(), false);
            return false;
        }
        let key = format!("fn:{}", name);
        let p = self.func_body_pure(&key, &f.body, visiting);
        self.pure_cache.insert(name.to_string(), p);
        p
    }

    /// Purity of a statement body, guarding against recursion via `visiting`.
    fn func_body_pure(&mut self, key: &str, body: &[Stmt], visiting: &mut std::collections::HashSet<String>) -> bool {
        if visiting.contains(key) {
            return true; // recursive call: assume pure within the cycle
        }
        visiting.insert(key.to_string());
        let p = self.stmts_pure(body, visiting);
        visiting.remove(key);
        p
    }

    fn stmts_pure(&mut self, stmts: &[Stmt], visiting: &mut std::collections::HashSet<String>) -> bool {
        stmts.iter().all(|s| self.stmt_pure(s, visiting))
    }

    fn stmt_pure(&mut self, s: &Stmt, visiting: &mut std::collections::HashSet<String>) -> bool {
        match s {
            Stmt::Print(_) | Stmt::Yield(_) => false,
            Stmt::Break | Stmt::Continue => true,
            Stmt::Assign(_, e) | Stmt::Return(e) | Stmt::Throw(e) | Stmt::Expr(e) => self.expr_pure(e, visiting),
            Stmt::IndexAssign(_, idxs, e) => idxs.iter().all(|i| self.expr_pure(i, visiting)) && self.expr_pure(e, visiting),
            Stmt::Try(body, _, handler) => self.stmts_pure(body, visiting) && self.stmts_pure(handler, visiting),
            Stmt::If(c, t, e) => self.expr_pure(c, visiting) && self.stmts_pure(t, visiting) && self.stmts_pure(e, visiting),
            Stmt::While(c, b) => self.expr_pure(c, visiting) && self.stmts_pure(b, visiting),
            Stmt::For(_, it, b) => self.expr_pure(it, visiting) && self.stmts_pure(b, visiting),
        }
    }

    /// Evaluate one expression against a scope (entry point for parallel comp workers).
    pub(crate) fn eval_in(&mut self, e: &AExpr, scope: &mut Scope) -> ER<Value> {
        self.eval(e, scope)
    }

    /// `parallel([fn_name, ...])` — run several 0-arg user functions
    /// concurrently, returning their results in listed order.
    fn par_run(&mut self, args: Vec<Value>) -> ER<Value> {
        if args.len() != 1 {
            return Err(Signal::Error("parallel expects ([fn_name, ...])".into()));
        }
        let names = builtins::iterate(&args[0]).map_err(Signal::from)?;
        let mut jobs = Vec::with_capacity(names.len());
        for n in &names {
            let name = n.as_str().map_err(Signal::from)?.to_string();
            jobs.push((name, Vec::new()));
        }
        let (vals, outs) = par_calls(Arc::clone(&self.prog), jobs)?;
        self.output.extend(outs);
        Ok(Value::list(vals))
    }

    fn construct(&mut self, class_name: &str, args: Vec<Value>) -> ER<Value> {
        let class = Arc::clone(self.prog.classes.get(class_name).expect("checked by caller"));
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("__class__".to_string(), Value::str(class_name));
        let inst = Value::map(fields);
        if let Some(init) = class.methods.get("init") {
            let init = Arc::clone(init);
            let mut local = Scope::default();
            local.insert("self".into(), inst.clone());
            for (p, v) in init.params.iter().skip(1).zip(args) {
                local.insert(p.clone(), v);
            }
            self.exec_block(&init.body, &mut local)?;
            Ok(local.get("self").cloned().unwrap_or(inst))
        } else {
            Ok(inst)
        }
    }
}

/// Insert or update a variable without cloning the key `String` when the
/// variable already exists (the common case in loops).
#[inline]
fn assign(scope: &mut Scope, name: &str, v: Value) {
    match scope.get_mut(name) {
        Some(slot) => *slot = v,
        None => {
            scope.insert(name.to_string(), v);
        }
    }
}

/// Builtins (and interpreter intercepts) that have side effects or
/// non-deterministic ordering, so a comprehension body using them must NOT be
/// auto-parallelized. Pure computations (math/string/list/map/vector/redact/
/// anonymize/…) are absent and therefore allowed. `filter`/`reduce` run a
/// *named* function we don't resolve here, so they stay conservative.
const IMPURE_BUILTINS: &[&str] = &[
    "sh", "sh_timeout", "psh", "auto_fix", "tool_run", "which", "mp_map", "proc_input", "tcp_request",
    "tcp_line", "http_get", "trace", "pmap", "amap", "parallel", "filter", "reduce", "now_ms",
    "read_file", "write_file", "append_file",
];

/// Whether transparent auto-parallelization is enabled (default on; set
/// `NEXUS_AUTO_PAR=0` to force everything serial).
fn auto_par_enabled() -> bool {
    !matches!(std::env::var("NEXUS_AUTO_PAR").as_deref(), Ok("0") | Ok("off") | Ok("false"))
}

/// Evaluate a comprehension body across worker threads, preserving input order.
/// Each thread owns a throwaway [`Interp`] and a private clone of the captured
/// scope; results are reassembled by index so the outcome is identical to the
/// serial path.
fn par_eval_comp(
    prog: Arc<Program>,
    base_scope: Scope,
    var: String,
    elem: AExpr,
    cond: Option<Box<AExpr>>,
    items: Vec<Value>,
) -> ER<Vec<Value>> {
    let n = items.len();
    let nthreads = nexus_accel::cores().clamp(1, n);
    let chunk = n.div_ceil(nthreads);
    let base = Arc::new(base_scope);
    let elem = Arc::new(elem);
    let cond = Arc::new(cond);
    let mut indexed: Vec<(usize, Value)> = items.into_iter().enumerate().collect();
    let mut handles = Vec::new();
    while !indexed.is_empty() {
        let take = chunk.min(indexed.len());
        let ch: Vec<(usize, Value)> = indexed.drain(0..take).collect();
        let prog = Arc::clone(&prog);
        let (base, elem, cond, var) = (Arc::clone(&base), Arc::clone(&elem), Arc::clone(&cond), var.clone());
        handles.push(std::thread::spawn(move || -> ER<Vec<(usize, Value)>> {
            let mut w = Interp::from_parts(prog);
            let mut scope = (*base).clone(); // one private scope per thread, reused
            let mut out = Vec::with_capacity(ch.len());
            for (i, item) in ch {
                assign(&mut scope, &var, item);
                if let Some(cc) = cond.as_ref() {
                    if !w.eval_in(cc, &mut scope)?.truthy() {
                        continue;
                    }
                }
                out.push((i, w.eval_in(&elem, &mut scope)?));
            }
            Ok(out)
        }));
    }
    let mut collected: Vec<(usize, Value)> = Vec::with_capacity(n);
    for h in handles {
        match h.join() {
            Ok(Ok(part)) => collected.extend(part),
            Ok(Err(sig)) => return Err(sig),
            Err(_) => return Err(Signal::Error("auto-parallel worker panicked".into())),
        }
    }
    collected.sort_by_key(|(i, _)| *i);
    Ok(collected.into_iter().map(|(_, v)| v).collect())
}

/// Run a batch of `(fn_name, args)` jobs across worker threads, each thread
/// owning a throwaway [`Interp`] over the shared definitions. Results and any
/// captured `print` output are reassembled in the original job order so the
/// outcome is deterministic regardless of how work was scheduled.
fn par_calls(prog: Arc<Program>, jobs: Vec<(String, Vec<Value>)>) -> ER<(Vec<Value>, Vec<String>)> {
    let n = jobs.len();
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let nthreads = nexus_accel::cores().clamp(1, n);
    let chunk = n.div_ceil(nthreads);
    nexus_core::trace::record(
        nexus_core::trace::Level::Debug,
        "exec.parallel",
        None,
        "fan out",
        &[("jobs", &n.to_string()), ("threads", &nthreads.to_string())],
    );
    let mut indexed: Vec<(usize, (String, Vec<Value>))> = jobs.into_iter().enumerate().collect();
    let mut handles = Vec::new();
    while !indexed.is_empty() {
        let take = chunk.min(indexed.len());
        let ch: Vec<(usize, (String, Vec<Value>))> = indexed.drain(0..take).collect();
        let prog = Arc::clone(&prog);
        handles.push(std::thread::spawn(move || -> ER<Vec<(usize, Value, Vec<String>)>> {
            let mut out = Vec::with_capacity(ch.len());
            for (idx, (name, argv)) in ch {
                let mut w = Interp::from_parts(Arc::clone(&prog));
                let v = w.call(&name, argv)?;
                out.push((idx, v, w.output));
            }
            Ok(out)
        }));
    }
    let mut collected: Vec<(usize, Value, Vec<String>)> = Vec::with_capacity(n);
    for h in handles {
        match h.join() {
            Ok(Ok(part)) => collected.extend(part),
            Ok(Err(sig)) => return Err(sig),
            Err(_) => return Err(Signal::Error("parallel worker thread panicked".into())),
        }
    }
    collected.sort_by_key(|(i, _, _)| *i);
    let mut outputs = Vec::new();
    let values = collected
        .into_iter()
        .map(|(_, v, o)| {
            outputs.extend(o);
            v
        })
        .collect();
    Ok((values, outputs))
}

/// Shallow cell count of a value, for memory-pressure accounting.
fn cells(v: &Value) -> usize {
    match v {
        Value::List(l) => l.len(),
        Value::Map(m) => m.len(),
        Value::Str(s) => s.len(),
        Value::Stream(_) | Value::Num(_) | Value::Bool(_) | Value::Nil => 1,
    }
}

fn check_arity(name: &str, params: &[String], got: usize) -> ER<()> {
    if params.len() != got {
        Err(Signal::Error(format!(
            "function `{}` expects {} args, got {}",
            name,
            params.len(),
            got
        )))
    } else {
        Ok(())
    }
}
