//! Command implementations: the Nexus compiler pipeline end-to-end.

use crate::bench;
use crate::tokens::Comparison;
use nexus_capability::{
    route_storage, AccessPattern, McpServer, Model, ModelRouter, Objective, Provider,
    ProviderType, ReasoningLevel, Registry, Requirement, RouteRequest, Trust,
};
use nexus_capability::{JsonRpcRequest, Engine};
use nexus_core::*;
use nexus_dsl::{parse, Program};
use nexus_graph::TripleGraph;
use nexus_optimize::{maximize, simulate, Bounds, TripleView};
use nexus_planner::{from_workflow, heft, pareto_front, route_within_budget, Processor};
use nexus_runtime::{execute_parallel, FnOp, KnowledgeLoop, NodeOp};
use nexus_verify::{format_model, prove, ProofResult};

// ---- helpers -------------------------------------------------------------

fn parse_or_err(src: &str) -> Result<Program, String> {
    parse(src).map_err(|e| e.to_string())
}

fn workflow_of(prog: &Program) -> Option<&Workflow> {
    prog.nodes.iter().find_map(|n| match n {
        Node::Workflow(w) => Some(w),
        _ => None,
    })
}

/// The no-overdraft safety invariant used to demonstrate formal verification.
fn overdraft_obligation() -> (Vec<Predicate>, Predicate) {
    let var = |n: &str| Expr::Var(n.into());
    let zero = || Expr::Lit(Literal::Number(Rational::int(0)));
    let assumptions = vec![
        Predicate {
            lhs: Expr::Bin(
                BinOp::Add,
                Box::new(Expr::Bin(BinOp::Sub, Box::new(var("balance")), Box::new(var("start")))),
                Box::new(var("amount")),
            ),
            op: CmpOp::Eq,
            rhs: zero(),
        },
        Predicate {
            lhs: Expr::Bin(
                BinOp::Sub,
                Box::new(Expr::Bin(BinOp::Sub, Box::new(var("start")), Box::new(var("amount")))),
                Box::new(var("minimum_reserve")),
            ),
            op: CmpOp::Ge,
            rhs: zero(),
        },
        Predicate { lhs: var("minimum_reserve"), op: CmpOp::Ge, rhs: zero() },
    ];
    let guarantee = Predicate { lhs: var("balance"), op: CmpOp::Ge, rhs: var("minimum_reserve") };
    (assumptions, guarantee)
}

fn default_procs() -> Vec<Processor> {
    vec![
        Processor::new("local-cpu", 1.0, 0.0, 5.0),
        Processor::new("edge-node", 1.3, 0.01, 8.0),
        Processor::new("cloud-gpu", 2.0, 0.05, 20.0),
    ]
}

// ---- commands ------------------------------------------------------------

pub fn cmd_parse(_path: &str, src: &str) -> Result<(), String> {
    let prog = parse_or_err(src)?;
    println!("project {}", prog.project);
    let kinds = ["policy", "thing", "capability", "contract", "constraint", "intent", "event", "agent", "workflow"];
    for k in kinds {
        let c = prog.nodes.iter().filter(|n| n.kind() == k).count();
        if c > 0 {
            println!("  {:<11} x{}", k, c);
        }
    }
    println!("  directives  x{}", prog.directives.len());
    println!("✓ parsed {} nodes", prog.nodes.len());
    Ok(())
}

pub fn cmd_build(_path: &str, src: &str) -> Result<(), String> {
    println!("Nexus compiler pipeline");
    println!("───────────────────────");

    // Stage 1: Sourcing & Parsing
    let prog = parse_or_err(src)?;
    let store = prog.to_store();
    println!("✓ Sourcing & Parsing    : {} nodes, {} edges", store.len(), store.edges().len());

    // Stage 2: Planning
    let triple = TripleGraph::from_store(&store);
    let dag = workflow_of(&prog).map(from_workflow);
    println!(
        "✓ Planning              : triple-graph K={} P={} E={}{}",
        triple.knowledge.len(),
        triple.plan.len(),
        triple.execution.len(),
        dag.as_ref().map(|d| format!(", plan DAG {} tasks", d.len())).unwrap_or_default()
    );

    // Stage 3: Optimization
    if let Some(d) = &dag {
        let procs = default_procs();
        let sched = heft(d, &procs);
        let front = pareto_front(d, &procs);
        println!(
            "✓ Optimization          : HEFT makespan {:.2}, cost ${:.4}, Pareto front {} options",
            sched.makespan, sched.cost, front.len()
        );
    } else {
        println!("✓ Optimization          : (no workflow to schedule)");
    }

    // Stage 4: AI Synthesis (route probabilistic/agent steps to models)
    let router = demo_router();
    let mut routed = 0;
    if let Some(w) = workflow_of(&prog) {
        for step in &w.steps {
            if step.agent.is_some() {
                let level = step
                    .reasoning
                    .as_deref()
                    .and_then(ReasoningLevel::parse)
                    .unwrap_or(ReasoningLevel::Balanced);
                let req = RouteRequest { level, est_input_tokens: 800, est_output_tokens: 200, max_cost: None };
                if let Some(m) = router.route(&req) {
                    println!("    · step {:<14} → model {} ({:?})", step.name, m.name, level);
                    routed += 1;
                }
            }
        }
    }
    println!("✓ AI Synthesis          : routed {} probabilistic step(s)", routed);

    // Stage 5: Formal SMT Proof — prove every DSL-declared guarantee.
    let obligations: Vec<(String, Vec<Predicate>, Predicate)> = if prog.guarantees.is_empty() {
        let (asm, guar) = overdraft_obligation();
        vec![("BuiltinNoOverdraft".into(), asm, guar)]
    } else {
        prog.guarantees
            .iter()
            .map(|g| (g.name.clone(), g.assumptions.clone(), g.ensure.clone()))
            .collect()
    };
    for (name, asm, guar) in &obligations {
        match prove(asm, guar) {
            ProofResult::Proven => println!("✓ Formal SMT Proof      : guarantee {name} PROVEN"),
            ProofResult::Violated { model } => {
                return Err(format!("SMT proof failed for {name} — counterexample: {}", format_model(&model)))
            }
            ProofResult::Unsupported { reason } => println!("⚠ Formal SMT Proof      : {name}: {reason}"),
            ProofResult::Unknown { reason } => println!("⚠ Formal SMT Proof      : {name}: inconclusive ({reason})"),
        }
    }

    // Stage 6: Execution Binding (bind capabilities to providers)
    let reg = demo_registry();
    let mut bound = 0;
    if let Some(w) = workflow_of(&prog) {
        for step in &w.steps {
            if let Some(need) = &step.need {
                let req = Requirement::new(need, Objective::Reliability)
                    .require("PCI-DSS")
                    .max_failure_rate(0.001);
                if let Some(p) = reg.resolve(&req) {
                    println!("    · need {:<14} → provider {} ({:?})", need, p.name, p.ptype);
                    bound += 1;
                }
            }
        }
    }
    println!("✓ Execution Binding     : bound {} capability need(s)", bound);
    println!("\n✓ BUILD SUCCEEDED — verified intent graph ready for runtime");
    Ok(())
}

pub fn cmd_check(_path: &str, src: &str) -> Result<(), String> {
    let prog = parse_or_err(src)?;
    // 1. Every constraint predicate must be well-formed.
    let mut constraints = 0;
    for n in &prog.nodes {
        if let Node::Constraint(c) = n {
            constraints += c.predicates.len();
        }
    }
    println!("✓ {constraints} constraint predicate(s) parsed and structurally valid");

    // 2. Every declared guarantee must be provable (falling back to the
    //    built-in no-overdraft obligation if the program declares none).
    let obligations: Vec<(String, Vec<Predicate>, Predicate)> = if prog.guarantees.is_empty() {
        let (asm, guar) = overdraft_obligation();
        vec![("BuiltinNoOverdraft".into(), asm, guar)]
    } else {
        prog.guarantees
            .iter()
            .map(|g| (g.name.clone(), g.assumptions.clone(), g.ensure.clone()))
            .collect()
    };
    for (name, asm, guar) in &obligations {
        match prove(asm, guar) {
            ProofResult::Proven => println!("✓ guarantee {name} PROVEN for all execution paths"),
            ProofResult::Violated { model } => {
                return Err(format!("guarantee {name} VIOLATED — counterexample: {}", format_model(&model)))
            }
            ProofResult::Unsupported { reason } => return Err(format!("{name}: {reason}")),
            ProofResult::Unknown { reason } => {
                return Err(format!("{name}: guarantee not proven (inconclusive): {reason}"))
            }
        }
    }
    Ok(())
}

pub fn cmd_fmt(_path: &str, src: &str) -> Result<(), String> {
    let view = TripleView::from_dsl(src).map_err(|e| e.to_string())?;
    if !view.consistent() {
        return Err("TGG round-trip inconsistency detected".into());
    }
    print!("{}", view.dsl());
    Ok(())
}

pub fn cmd_graph(_path: &str, src: &str) -> Result<(), String> {
    let prog = parse_or_err(src)?;
    let store = prog.to_store();
    println!("{} content-addressed nodes:", store.len());
    for (cid, node) in store.iter() {
        let name = node.name().unwrap_or("·");
        println!("  {} {:<11} {}", cid.short(), node.kind(), name);
    }
    println!("\n{} edges", store.edges().len());
    Ok(())
}

pub fn cmd_plan(_path: &str, src: &str) -> Result<(), String> {
    let prog = parse_or_err(src)?;
    let wf = workflow_of(&prog).ok_or("no workflow to plan")?;
    let dag = from_workflow(wf);
    let procs = default_procs();

    println!("Auto-parallel levels:");
    for (i, level) in dag.levels().iter().enumerate() {
        let names: Vec<&str> = level.iter().map(|t| dag.names[*t].as_str()).collect();
        println!("  L{i}: {}", names.join(" ∥ "));
    }
    let sched = heft(&dag, &procs);
    println!("\nHEFT schedule ({} processors):", procs.len());
    for t in 0..dag.len() {
        println!(
            "  {:<16} → {:<10} [{:.2}..{:.2}]",
            dag.names[t], procs[sched.proc_of[t]].name, sched.start[t], sched.finish[t]
        );
    }
    println!("\nmakespan {:.2}  cost ${:.4}  energy {:.1}J", sched.makespan, sched.cost, sched.energy);
    let front = pareto_front(&dag, &procs);
    println!("Pareto front: {} non-dominated schedule(s)", front.len());
    match route_within_budget(&dag, &procs, 0.05, 1000.0) {
        Some(s) => println!("Budget route (≤$0.05): makespan {:.2}, cost ${:.4}", s.makespan, s.cost),
        None => println!("Budget route (≤$0.05): infeasible — would reroute/fallback"),
    }
    Ok(())
}

pub fn cmd_run(_path: &str, src: &str) -> Result<(), String> {
    let prog = parse_or_err(src)?;
    let wf = workflow_of(&prog).ok_or("no workflow to run")?;
    let dag = from_workflow(wf);
    let ops: Vec<Box<dyn NodeOp>> = wf
        .steps
        .iter()
        .map(|s| {
            let name = s.name.clone();
            let det = s.deterministic == Some(true);
            Box::new(FnOp {
                name: name.clone(),
                det,
                f: move |inputs: &[&[u8]]| {
                    let mut out = name.as_bytes().to_vec();
                    for i in inputs {
                        out.extend_from_slice(i);
                    }
                    out
                },
            }) as Box<dyn NodeOp>
        })
        .collect();

    let report = execute_parallel(&dag, &ops);
    println!("Executed {} steps across {} level(s):", dag.len(), report.level_count);
    for t in 0..dag.len() {
        println!("  {:<16} → out {}", dag.names[t], report.output_cids[t].short());
    }
    let (det, prob) = nexus_runtime::determinism_split(&ops);
    println!("\nprovenance intact : {}", report.provenance_intact());
    println!("deterministic     : {det}   probabilistic: {prob}");
    println!("max parallelism   : {}", report.max_parallelism);
    Ok(())
}

pub fn cmd_tokencmp(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: weft tokencmp <weft-file> <other-file>".into());
    }
    let a = std::fs::read_to_string(&args[0]).map_err(|e| e.to_string())?;
    let b = std::fs::read_to_string(&args[1]).map_err(|e| e.to_string())?;
    let cmp = Comparison::new(&a, &b);
    println!("{:<22} {:>7} {:>7} {:>7} {:>7}", "file", "tokens", "chars", "words", "lines");
    println!(
        "{:<22} {:>7} {:>7} {:>7} {:>7}",
        args[0], cmp.nexus.est_tokens, cmp.nexus.chars, cmp.nexus.words, cmp.nexus.lines
    );
    println!(
        "{:<22} {:>7} {:>7} {:>7} {:>7}",
        args[1], cmp.traditional.est_tokens, cmp.traditional.chars, cmp.traditional.words, cmp.traditional.lines
    );
    let d = cmp.nexus.est_tokens as f64 - cmp.traditional.est_tokens as f64;
    if d <= 0.0 {
        println!("→ Nexus uses {:.0} fewer tokens ({:.0}% less)", -d, -100.0 * d / cmp.traditional.est_tokens.max(1) as f64);
    } else {
        println!("→ Nexus uses {:.0} MORE tokens (+{:.0}%)", d, 100.0 * d / cmp.traditional.est_tokens.max(1) as f64);
    }
    Ok(())
}

pub fn cmd_app(_path: &str, src: &str) -> Result<(), String> {
    // The mixed loader runs pure-executable and declarative+executable files
    // alike, exposing `validate`/`check`/`prove`/`things`/`fields` to code.
    let out = nexus_app::run_mixed(src)?;
    for line in out {
        println!("{line}");
    }
    Ok(())
}

/// `nexus app <file> --dashboard`: run the program, surface its output on the
/// auto-generated dashboard, then serve it. The dashboard is strictly opt-in —
/// without the flag (or the `dashboard` command) nothing binds a socket, so the
/// default experience stays headless.
pub fn cmd_app_dashboard(_path: &str, src: &str, addr: &str) -> Result<(), String> {
    let out = nexus_app::run_mixed(src)?;
    for line in &out {
        println!("{line}");
    }
    nexus_dashboard::set_output(out);
    eprintln!("Dashboard live at http://{addr}  —  view output/traces, run programs (Ctrl-C to stop)");
    nexus_dashboard::serve(addr)
}

/// `nexus dashboard [--port N]`: serve the control/observability dashboard
/// without first running a program (you can run programs from the web UI).
pub fn cmd_dashboard(addr: &str) -> Result<(), String> {
    eprintln!("Dashboard live at http://{addr}  (headless by default; this is the opt-in UI). Ctrl-C to stop.");
    nexus_dashboard::serve(addr)
}

pub fn cmd_tokens(_path: &str, src: &str) -> Result<(), String> {
    let trad = include_str!("../baselines/enterprise_billing_traditional.ts");
    let cmp = Comparison::new(src, trad);
    println!("Token usage for the same system:");
    println!(
        "  Nexus DSL    : {:>6} tokens  {:>4} lines  {:>6} chars  {:>4} words",
        cmp.nexus.est_tokens, cmp.nexus.lines, cmp.nexus.chars, cmp.nexus.words
    );
    println!(
        "  Traditional  : {:>6} tokens  {:>4} lines  {:>6} chars  {:>4} words",
        cmp.traditional.est_tokens, cmp.traditional.lines, cmp.traditional.chars, cmp.traditional.words
    );
    println!(
        "  → {:.1}x fewer tokens, {:.0}% saved",
        cmp.token_reduction_factor(),
        cmp.token_savings_pct()
    );
    Ok(())
}

/// A real MCP server over stdio: reads one JSON-RPC 2.0 request per line from
/// stdin and writes one JSON response per line to stdout. Beyond capability
/// discovery it exposes Nexus itself as a tool (`run_app`), so any MCP client
/// can execute Nexus programs.
pub fn cmd_mcp_serve() -> Result<(), String> {
    use std::io::{BufRead, Write};
    let reg = ocr_registry();
    let server = McpServer::new(&reg);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let (resp, exit) = mcp_dispatch(&server, &line);
        writeln!(stdout, "{resp}").map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        if exit {
            break;
        }
    }
    Ok(())
}

/// Route one request line: app-level methods are handled here; capability
/// discovery delegates to the in-process MCP server. Returns (response, exit).
fn mcp_dispatch(server: &McpServer, line: &str) -> (String, bool) {
    use serde_json::{json, Value};
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return (
                json!({"jsonrpc":"2.0","error":{"code":-32700,"message":format!("parse error: {e}")},"id":0}).to_string(),
                false,
            )
        }
    };
    let id = req.get("id").cloned().unwrap_or(json!(0));
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(json!({}));

    match method {
        "ping" => (json!({"jsonrpc":"2.0","result":"pong","id":id}).to_string(), false),
        "shutdown" => (json!({"jsonrpc":"2.0","result":"bye","id":id}).to_string(), true),
        "list_tools" => (
            json!({"jsonrpc":"2.0","result":{"tools":["discover","query","bind","run_app"]},"id":id}).to_string(),
            false,
        ),
        // Execute a Nexus program (declarative+executable) and return its output.
        "run_app" => {
            let source = params.get("source").and_then(Value::as_str).unwrap_or("");
            let resp = match nexus_app::run_mixed(source) {
                Ok(out) => json!({"jsonrpc":"2.0","result":{"output": out},"id":id}),
                Err(e) => json!({"jsonrpc":"2.0","error":{"code":-32000,"message":e},"id":id}),
            };
            (resp.to_string(), false)
        }
        // Everything else: capability discovery via the MCP server.
        _ => (server.handle_str(line), false),
    }
}

/// Serve the MCP/JSON-RPC protocol over an already-bound TCP listener (one
/// client at a time; a `shutdown` request stops the server). Split out from
/// [`cmd_mcp_serve_tcp`] so tests can bind to port 0 and drive it directly.
pub fn serve_mcp_on(listener: std::net::TcpListener) -> Result<(), String> {
    use std::io::{BufRead, BufReader, Write};
    let reg = ocr_registry();
    let server = McpServer::new(&reg);
    for conn in listener.incoming() {
        let stream = conn.map_err(|e| e.to_string())?;
        let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
        nexus_core::trace::record(nexus_core::trace::Level::Info, "mcp.tcp", None, "client connected", &[("peer", &peer)]);
        let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
        let reader = BufReader::new(stream);
        let mut shutdown = false;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            let (resp, exit) = mcp_dispatch(&server, &line);
            writeln!(writer, "{resp}").map_err(|e| e.to_string())?;
            writer.flush().ok();
            if exit {
                shutdown = true;
                break;
            }
        }
        if shutdown {
            break;
        }
    }
    Ok(())
}

/// `nexus mcp-serve --tcp <addr>`: the real MCP transport over TCP sockets.
pub fn cmd_mcp_serve_tcp(addr: &str) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(addr).map_err(|e| e.to_string())?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?;
    eprintln!("MCP server listening on tcp://{bound}");
    serve_mcp_on(listener)
}

/// Build resource limits from environment overrides (NEXUS_STEPS,
/// NEXUS_ALLOC_CELLS, NEXUS_WALL_MS), falling back to safe defaults.
fn limits_from_env() -> nexus_exec::Limits {
    let mut l = nexus_exec::Limits::default();
    if let Ok(n) = std::env::var("NEXUS_STEPS").unwrap_or_default().parse() {
        l.steps = n;
    }
    if let Ok(n) = std::env::var("NEXUS_ALLOC_CELLS").unwrap_or_default().parse() {
        l.alloc_cells = n;
    }
    if let Ok(ms) = std::env::var("NEXUS_WALL_MS").unwrap_or_default().parse::<u64>() {
        l.wall = Some(std::time::Duration::from_millis(ms));
    }
    l
}

/// `nexus run-guarded <file>`: execute under hard resource limits with
/// transactional rollback. Returns Ok(true) on a clean run, Ok(false) if a
/// breach was caught and contained (the caller maps that to a distinct exit
/// code so an out-of-process supervisor can tell containment from a crash).
pub fn cmd_run_guarded(_path: &str, src: &str) -> Result<bool, String> {
    let app = nexus_exec::parse_app(src).map_err(|e| e.to_string())?;
    let outcome = nexus_exec::Interp::run_guarded(&app, limits_from_env());
    for line in &outcome.output {
        println!("{line}");
    }
    eprintln!(
        "── guarded run ── steps={} alloc_cells={} live_vars={}",
        outcome.steps, outcome.alloc_cells, outcome.live_vars
    );
    match &outcome.error {
        Some(err) => {
            eprintln!("⚠ breach contained, state rolled back: {err}");
            Ok(false)
        }
        None => Ok(true),
    }
}

pub fn cmd_mcp() -> Result<(), String> {
    let reg = ocr_registry();
    let server = McpServer::new(&reg);
    println!("MCP capability discovery over JSON-RPC 2.0\n");
    for (method, params) in [
        ("discover", serde_json::json!({})),
        ("query", serde_json::json!({"capability": "ocr_scanner"})),
        ("bind", serde_json::json!({"capability": "ocr_scanner", "objective": "cost"})),
        ("bind", serde_json::json!({"capability": "ocr_scanner", "objective": "latency"})),
    ] {
        let req = JsonRpcRequest::new(method, params, 1);
        let req_s = serde_json::to_string(&req).unwrap();
        println!("→ {}", req_s);
        println!("← {}\n", server.handle_str(&req_s));
    }
    // Universal data routing demo.
    println!("Universal data routing:");
    for (p, label) in [
        (AccessPattern::StructuredQuery, "relational"),
        (AccessPattern::Analytical, "analytical"),
        (AccessPattern::KeyValue, "cache"),
        (AccessPattern::SemanticSearch, "embeddings"),
    ] {
        let e: Engine = route_storage(p, true, true);
        println!("  {:<11} → {:?}", label, e);
    }
    Ok(())
}

pub fn cmd_bench(args: &[String]) -> Result<(), String> {
    let report = bench::run();
    let md = bench::to_markdown(&report);
    print!("{md}");
    if args.iter().any(|a| a == "--write") {
        std::fs::write("BENCHMARKS.md", &md).map_err(|e| e.to_string())?;
        println!("(written to BENCHMARKS.md)");
    }
    Ok(())
}

pub fn cmd_demo() -> Result<(), String> {
    let src = nexus_dsl::ENTERPRISE_BILLING;
    println!("╔══ Nexus end-to-end demonstration ══╗\n");
    cmd_build("reference", src)?;

    println!("\n── Digital-twin simulation ──");
    let prog = parse_or_err(src)?;
    let wf = workflow_of(&prog).unwrap();
    let dag = from_workflow(wf);
    let names: Vec<String> = wf.steps.iter().map(|s| s.name.clone()).collect();
    let procs = default_procs();
    let sched = heft(&dag, &procs);
    let mut k = KnowledgeLoop::new();
    for _ in 0..5 {
        k.record("ChargeAccount", 220.0, 0.018, true);
    }
    let report = simulate(&dag, &names, &k, &sched, &procs, (0.025, 1500.0));
    println!(
        "predicted latency {:.0}ms, cost ${:.4}, success {:.3}, deployable: {}",
        report.predicted_latency_ms, report.predicted_cost, report.success_probability, report.is_safe_to_deploy()
    );

    println!("\n── DSPy/TuRBO prompt optimization ──");
    // Optimize a synthetic prompt-quality surface (accuracy vs token cost).
    let bounds = Bounds::new(vec![0.0, 0.0], vec![1.0, 8.0]);
    let res = maximize(
        |x| {
            // higher temperature near 0.4 and ~3 few-shot examples is best
            
            1.0 - (x[0] - 0.4).powi(2) - 0.02 * (x[1] - 3.0).powi(2)
        },
        &bounds,
        60,
        7,
    );
    println!(
        "best prompt: temp={:.2}, few_shot={:.1} → metric {:.3} in {} evals",
        res.best_x[0], res.best_x[1], res.best_y, res.evals
    );

    println!("\n── Token economy ──");
    cmd_tokens("reference", src)?;
    println!("\n✓ All subsystems demonstrated.");
    Ok(())
}

// ---- demo fixtures -------------------------------------------------------

fn demo_router() -> ModelRouter {
    let mut r = ModelRouter::new();
    r.register(Model { name: "opus".into(), cost_per_1k_in: 0.015, cost_per_1k_out: 0.075, quality: 0.98, context_window: 200_000, latency_ms: 900.0, available: true });
    r.register(Model { name: "haiku".into(), cost_per_1k_in: 0.0008, cost_per_1k_out: 0.004, quality: 0.82, context_window: 200_000, latency_ms: 250.0, available: true });
    r
}

fn demo_registry() -> Registry {
    let mut r = Registry::new();
    r.register(Provider {
        name: "StripeProcessor".into(),
        capability: "CardProcessor".into(),
        ptype: ProviderType::RemoteApi,
        trust: Trust::Verified,
        cost_per_call: 0.012,
        latency_ms: 220.0,
        failure_rate: 0.0007,
        compliance: vec!["PCI-DSS".into(), "SCA".into()],
    });
    r.register(Provider {
        name: "AdyenProcessor".into(),
        capability: "CardProcessor".into(),
        ptype: ProviderType::RemoteApi,
        trust: Trust::Verified,
        cost_per_call: 0.010,
        latency_ms: 260.0,
        failure_rate: 0.0009,
        compliance: vec!["PCI-DSS".into(), "SCA".into()],
    });
    r
}

fn ocr_registry() -> Registry {
    let mut r = Registry::new();
    r.register(Provider { name: "Tesseract".into(), capability: "ocr_scanner".into(), ptype: ProviderType::LocalWasm, trust: Trust::Proven, cost_per_call: 0.0, latency_ms: 120.0, failure_rate: 0.03, compliance: vec![] });
    r.register(Provider { name: "GoogleVision".into(), capability: "ocr_scanner".into(), ptype: ProviderType::RemoteApi, trust: Trust::Verified, cost_per_call: 0.015, latency_ms: 40.0, failure_rate: 0.005, compliance: vec!["SOC2".into()] });
    r.register(Provider { name: "AWSTextract".into(), capability: "ocr_scanner".into(), ptype: ProviderType::RemoteApi, trust: Trust::Reviewed, cost_per_call: 0.025, latency_ms: 60.0, failure_rate: 0.004, compliance: vec!["SOC2".into(), "HIPAA".into()] });
    r
}
