//! Semantic parser: blocks → typed graph nodes.
//!
//! Each top-level block is dispatched on its head keyword and projected into one
//! of the seven primitives (or a Workflow/Agent/Directive). The result is a
//! [`Program`] that can be lowered into a content-addressed [`GraphStore`].

use crate::block::{build_blocks, Block};
use crate::lexer::Tok;
use nexus_core::*;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error (line {}): {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

type PResult<T> = Result<T, ParseError>;

/// A formal safety obligation: prove `ensure` holds under all `assumptions`.
/// This is the DSL surface of the SMT proof pass (`guarantee … ensure …`).
#[derive(Debug, Clone)]
pub struct Guarantee {
    pub name: String,
    pub assumptions: Vec<Predicate>,
    pub ensure: Predicate,
}

/// A fully parsed Nexus program: a project name, its nodes, top-level
/// scheduler/observability directives, and formal guarantees, in source order.
#[derive(Debug, Clone)]
pub struct Program {
    pub project: String,
    pub nodes: Vec<Node>,
    pub directives: Vec<Directive>,
    pub guarantees: Vec<Guarantee>,
}

impl Program {
    /// Lower the program into a content-addressed store, wiring symbolic
    /// references (field policies, intent constraints, workflow steps, …) as
    /// typed edges.
    pub fn to_store(&self) -> GraphStore {
        let mut store = GraphStore::new();
        let mut cids = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            cids.push(store.insert(node.clone()));
        }
        for (node, &cid) in self.nodes.iter().zip(&cids) {
            wire_edges(&mut store, node, cid);
        }
        // Scheduler/observability directives are execution-graph nodes.
        for d in &self.directives {
            store.insert(Node::Directive(d.clone()));
        }
        store
    }
}

fn wire_edges(store: &mut GraphStore, node: &Node, cid: Cid) {
    let link = |store: &mut GraphStore, kind: &str, name: &str, label: &str| {
        if let Some(target) = store.cid_of(kind, name) {
            store.add_edge(cid, target, label);
        }
    };
    match node {
        Node::Thing(t) => {
            for f in &t.fields {
                if let Some(p) = &f.policy {
                    link(store, "policy", p, "field_policy");
                }
            }
        }
        Node::Intent(i) => {
            for c in &i.constraints {
                link(store, "constraint", c, "constrained_by");
            }
        }
        Node::Agent(a) => {
            for c in &a.constraints {
                link(store, "constraint", c, "constrained_by");
            }
        }
        Node::Workflow(w) => {
            for s in &w.steps {
                if let Some(i) = &s.intent {
                    link(store, "intent", i, "step_intent");
                }
                if let Some(a) = &s.agent {
                    link(store, "agent", a, "step_agent");
                }
                if let Some(c) = &s.contract {
                    link(store, "contract", c, "step_contract");
                }
                if let Some(n) = &s.need {
                    link(store, "capability", n, "step_need");
                }
            }
        }
        _ => {}
    }
}

/// Parse Nexus DSL source into a [`Program`].
pub fn parse(src: &str) -> PResult<Program> {
    let blocks = build_blocks(src).map_err(|e| ParseError { line: e.line, msg: e.msg })?;
    parse_blocks(&blocks)
}

/// Top-level declarative keywords. Used to partition mixed declarative+
/// executable files.
pub const DECL_KEYWORDS: &[&str] = &[
    "project", "policy", "thing", "capability", "contract", "constraint",
    "intent", "event", "agent", "workflow", "guarantee", "schedule", "observe",
    "optimize", "secure",
];

/// Parse a program from an already-built block forest (declarative blocks only).
pub fn parse_blocks(blocks: &[Block]) -> PResult<Program> {
    let mut project = String::new();
    let mut nodes = Vec::new();
    let mut directives = Vec::new();
    let mut guarantees = Vec::new();

    for b in blocks {
        let head = b.head().ok_or_else(|| err(b.line_no, "expected a keyword"))?;
        match head {
            "project" => project = ident_at(b, 1)?,
            "policy" => nodes.push(Node::Policy(parse_policy(b)?)),
            "thing" => nodes.push(Node::Thing(parse_thing(b)?)),
            "capability" => nodes.push(Node::Capability(parse_capability(b)?)),
            "contract" => nodes.push(Node::Contract(parse_contract(b)?)),
            "constraint" => nodes.push(Node::Constraint(parse_constraint(b)?)),
            "intent" => nodes.push(Node::Intent(parse_intent(b)?)),
            "event" => nodes.push(Node::Event(Event { name: ident_at(b, 1)? })),
            "agent" => nodes.push(Node::Agent(parse_agent(b)?)),
            "workflow" => nodes.push(Node::Workflow(parse_workflow(b)?)),
            "guarantee" => guarantees.push(parse_guarantee(b)?),
            "schedule" => directives.push(Directive::Schedule {
                target: ident_at(b, 1)?,
                mode: ident_at(b, 2)?,
            }),
            "observe" => directives.push(Directive::Observe(ident_at(b, 1)?)),
            "optimize" => directives.push(Directive::Optimize(ident_at(b, 1)?)),
            "secure" => directives.push(Directive::Secure(ident_at(b, 1)?)),
            other => return Err(err(b.line_no, &format!("unknown top-level keyword '{}'", other))),
        }
    }
    Ok(Program { project, nodes, directives, guarantees })
}

fn parse_guarantee(b: &Block) -> PResult<Guarantee> {
    let name = ident_at(b, 1)?;
    let mut assumptions = Vec::new();
    let mut ensure = None;
    for c in &b.children {
        match c.head() {
            Some("assume") => assumptions.push(parse_predicate(&c.toks[1..], c.line_no)?),
            Some("ensure") => ensure = Some(parse_predicate(&c.toks[1..], c.line_no)?),
            _ => return Err(err(c.line_no, "expected 'assume' or 'ensure'")),
        }
    }
    let ensure = ensure.ok_or_else(|| err(b.line_no, "guarantee requires an 'ensure' clause"))?;
    Ok(Guarantee { name, assumptions, ensure })
}

// ---- block parsers -------------------------------------------------------

fn parse_policy(b: &Block) -> PResult<Policy> {
    let name = ident_at(b, 1)?;
    let mut directives = Vec::new();
    for c in &b.children {
        let head = c.head().ok_or_else(|| err(c.line_no, "expected policy directive"))?;
        let d = match head {
            "restrict" => {
                // restrict <action> -> <target>
                let action = ident_at(c, 1)?;
                if c.toks.get(2) != Some(&Tok::Arrow) {
                    return Err(err(c.line_no, "expected '->' in restrict directive"));
                }
                let target = qual(&c.toks, 3, c.line_no)?.0;
                SecurityDirective::Restrict { action, target }
            }
            "encrypt" => SecurityDirective::Encrypt(qual_at(c, 1)?),
            "audit" => SecurityDirective::Audit(bool_at(c, 1)?),
            "trust" => SecurityDirective::Trust(ident_at(c, 1)?),
            "default_deny" => SecurityDirective::DefaultDeny(bool_at(c, 1)?),
            other => return Err(err(c.line_no, &format!("unknown policy directive '{}'", other))),
        };
        directives.push(d);
    }
    Ok(Policy { name, directives })
}

fn parse_thing(b: &Block) -> PResult<Thing> {
    let name = ident_at(b, 1)?;
    let implements = if b.toks.get(2) == Some(&Tok::Ident("implements".into())) {
        Some(ident_at(b, 3)?)
    } else {
        None
    };
    let mut fields = Vec::new();
    for c in &b.children {
        // <name> : <type> [policy <name>]
        let fname = as_ident(&c.toks[0], c.line_no)?;
        if c.toks.get(1) != Some(&Tok::Colon) {
            return Err(err(c.line_no, "expected ':' after field name"));
        }
        let mut idx = 2;
        let ty = parse_type(&c.toks, &mut idx, c.line_no)?;
        let mut policy = None;
        if c.toks.get(idx) == Some(&Tok::Ident("policy".into())) {
            policy = Some(as_ident(&c.toks[idx + 1], c.line_no)?);
        }
        fields.push(Field { name: fname, ty, policy });
    }
    Ok(Thing { name, implements, fields })
}

fn parse_capability(b: &Block) -> PResult<Capability> {
    let name = ident_at(b, 1)?;
    let mut signature = Signature::default();
    let mut guarantees = Vec::new();
    for c in &b.children {
        match c.head() {
            Some("signature") => {
                for line in &c.children {
                    let key = as_ident(&line.toks[0], line.line_no)?;
                    let pairs = parse_named_type_list(&line.toks[2..], line.line_no)?;
                    match key.as_str() {
                        "input" => signature.inputs = pairs,
                        "output" => signature.outputs = pairs,
                        other => {
                            return Err(err(line.line_no, &format!("unknown signature key '{}'", other)))
                        }
                    }
                }
            }
            Some("guarantee") => {
                for line in &c.children {
                    let (k, v) = parse_kv(line)?;
                    guarantees.push((k, v));
                }
            }
            _ => return Err(err(c.line_no, "expected 'signature' or 'guarantee'")),
        }
    }
    Ok(Capability { name, signature, guarantees })
}

fn parse_contract(b: &Block) -> PResult<Contract> {
    let name = ident_at(b, 1)?;
    let mut limits = Vec::new();
    for c in &b.children {
        limits.push(parse_kv(c)?);
    }
    Ok(Contract { name, limits })
}

fn parse_constraint(b: &Block) -> PResult<Constraint> {
    let name = ident_at(b, 1)?;
    let mut predicates = Vec::new();
    for c in &b.children {
        predicates.push(parse_predicate(&c.toks, c.line_no)?);
    }
    Ok(Constraint { name, predicates })
}

fn parse_intent(b: &Block) -> PResult<Intent> {
    let name = ident_at(b, 1)?;
    let mut inputs = Vec::new();
    let mut goals = Vec::new();
    let mut constraints = Vec::new();
    for c in &b.children {
        match c.head() {
            Some("input") => inputs = parse_named_type_list(&c.toks[2..], c.line_no)?,
            Some("goal") => {
                for g in &c.children {
                    goals.push(statement(&g.toks, g.line_no)?);
                }
            }
            Some("constraint") => constraints.push(ident_at(c, 1)?),
            _ => return Err(err(c.line_no, "unexpected line in intent")),
        }
    }
    Ok(Intent { name, inputs, goals, constraints })
}

fn parse_agent(b: &Block) -> PResult<Agent> {
    let name = ident_at(b, 1)?;
    let mut goals = Vec::new();
    let mut metric = None;
    let mut tools = Vec::new();
    let mut constraints = Vec::new();
    for c in &b.children {
        match c.head() {
            Some("goal") => {
                for g in &c.children {
                    goals.push(statement(&g.toks, g.line_no)?);
                }
            }
            Some("metric") => {
                let mname = ident_at(c, 1)?;
                let mut objectives = Vec::new();
                for o in &c.children {
                    let obj = match o.head() {
                        Some("maximize") => Objective::Maximize,
                        Some("minimize") => Objective::Minimize,
                        _ => return Err(err(o.line_no, "expected maximize/minimize")),
                    };
                    objectives.push((obj, statement(&o.toks[1..], o.line_no)?));
                }
                metric = Some(Metric { name: mname, objectives });
            }
            Some("tools") => tools = parse_ident_list(&c.toks[2..], c.line_no)?,
            Some("constraint") => constraints.push(ident_at(c, 1)?),
            _ => return Err(err(c.line_no, "unexpected line in agent")),
        }
    }
    Ok(Agent { name, goals, metric, tools, constraints })
}

fn parse_workflow(b: &Block) -> PResult<Workflow> {
    let name = ident_at(b, 1)?;
    let mut trigger = String::new();
    let mut steps = Vec::new();
    for c in &b.children {
        match c.head() {
            Some("trigger") => {
                // trigger on <Event.Ref>
                // trigger on <Event.Ref>
                trigger = qual(&c.toks, 2, c.line_no)?.0;
            }
            Some("step") => steps.push(parse_step(c)?),
            _ => return Err(err(c.line_no, "expected 'trigger' or 'step'")),
        }
    }
    Ok(Workflow { name, trigger, steps })
}

fn parse_step(b: &Block) -> PResult<Step> {
    let mut s = Step { name: ident_at(b, 1)?, ..Default::default() };
    for c in &b.children {
        match c.head() {
            Some("intent") => s.intent = Some(ident_at(c, 1)?),
            Some("agent") => s.agent = Some(ident_at(c, 1)?),
            Some("need") => s.need = Some(as_ident(&c.toks[2], c.line_no)?),
            Some("contract") => s.contract = Some(ident_at(c, 1)?),
            Some("deterministic") => s.deterministic = Some(bool_at(c, 1)?),
            Some("reasoning") => s.reasoning = Some(qual_at(c, 1)?),
            Some("tools") => s.tools = parse_ident_list(&c.toks[2..], c.line_no)?,
            Some("fallback") => s.fallback = Some(parse_fallback(c)?),
            _ => return Err(err(c.line_no, "unexpected line in step")),
        }
    }
    Ok(s)
}

fn parse_fallback(b: &Block) -> PResult<Fallback> {
    let mut fb = Fallback::default();
    for c in &b.children {
        match c.head() {
            Some("retry") => {
                fb.retry = Some(match &c.toks[1] {
                    Tok::Number(n) => n.parse().map_err(|_| err(c.line_no, "bad retry count"))?,
                    _ => return Err(err(c.line_no, "expected retry count")),
                });
            }
            Some("backoff") => fb.backoff = Some(ident_at(c, 1)?),
            Some("on_failure") => fb.on_failure = Some(ident_at(c, 1)?),
            _ => return Err(err(c.line_no, "unexpected line in fallback")),
        }
    }
    Ok(fb)
}

// ---- token helpers -------------------------------------------------------

fn err(line: usize, msg: &str) -> ParseError {
    ParseError { line, msg: msg.to_string() }
}

fn as_ident(t: &Tok, line: usize) -> PResult<String> {
    match t {
        Tok::Ident(s) => Ok(s.clone()),
        _ => Err(err(line, "expected identifier")),
    }
}

fn ident_at(b: &Block, idx: usize) -> PResult<String> {
    b.toks
        .get(idx)
        .ok_or_else(|| err(b.line_no, "missing identifier"))
        .and_then(|t| as_ident(t, b.line_no))
}

/// Read a qualified, dotted name (`Ident ('.' Ident)*`) starting at `start`,
/// returning the joined string and the index just past it.
fn qual(toks: &[Tok], mut i: usize, line: usize) -> PResult<(String, usize)> {
    let mut s = as_ident(toks.get(i).ok_or_else(|| err(line, "missing identifier"))?, line)?;
    i += 1;
    while toks.get(i) == Some(&Tok::Dot) {
        let part = as_ident(
            toks.get(i + 1).ok_or_else(|| err(line, "expected identifier after '.'"))?,
            line,
        )?;
        s.push('.');
        s.push_str(&part);
        i += 2;
    }
    Ok((s, i))
}

fn qual_at(b: &Block, idx: usize) -> PResult<String> {
    qual(&b.toks, idx, b.line_no).map(|(s, _)| s)
}

fn bool_at(b: &Block, idx: usize) -> PResult<bool> {
    match b.toks.get(idx) {
        Some(Tok::Ident(s)) if s == "true" => Ok(true),
        Some(Tok::Ident(s)) if s == "false" => Ok(false),
        _ => Err(err(b.line_no, "expected true/false")),
    }
}

/// Join a sequence of identifier tokens into a single statement string.
fn statement(toks: &[Tok], line: usize) -> PResult<String> {
    if toks.is_empty() {
        return Err(err(line, "expected statement"));
    }
    let mut parts = Vec::new();
    for t in toks {
        parts.push(as_ident(t, line)?);
    }
    Ok(parts.join(" "))
}

/// Parse a possibly-nested type starting at `*idx`, advancing it.
fn parse_type(toks: &[Tok], idx: &mut usize, line: usize) -> PResult<Ty> {
    let base = as_ident(toks.get(*idx).ok_or_else(|| err(line, "expected type"))?, line)?;
    *idx += 1;
    if toks.get(*idx) == Some(&Tok::LBracket) {
        *idx += 1;
        let inner = parse_type(toks, idx, line)?;
        if toks.get(*idx) != Some(&Tok::RBracket) {
            return Err(err(line, "expected ']' closing list type"));
        }
        *idx += 1;
        return Ok(Ty::List(Box::new(inner)));
    }
    Ok(Ty::parse(&base))
}

/// Parse `name: Type, name: Type, …`.
fn parse_named_type_list(toks: &[Tok], line: usize) -> PResult<Vec<(String, Ty)>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let name = as_ident(&toks[i], line)?;
        i += 1;
        if toks.get(i) != Some(&Tok::Colon) {
            return Err(err(line, "expected ':' after name"));
        }
        i += 1;
        let ty = parse_type(toks, &mut i, line)?;
        out.push((name, ty));
        if toks.get(i) == Some(&Tok::Comma) {
            i += 1;
        }
    }
    Ok(out)
}

/// Parse `[A, B, C]` of identifiers.
fn parse_ident_list(toks: &[Tok], line: usize) -> PResult<Vec<String>> {
    let mut out = Vec::new();
    for t in toks {
        match t {
            Tok::Ident(s) => out.push(s.clone()),
            Tok::Str(s) => out.push(s.clone()),
            Tok::LBracket | Tok::RBracket | Tok::Comma => {}
            _ => return Err(err(line, "expected identifier in list")),
        }
    }
    Ok(out)
}

/// Parse `key: <literal>` from a block line.
fn parse_kv(b: &Block) -> PResult<(String, Literal)> {
    let key = as_ident(&b.toks[0], b.line_no)?;
    if b.toks.get(1) != Some(&Tok::Colon) {
        return Err(err(b.line_no, "expected ':' in key/value"));
    }
    let lit = parse_literal(&b.toks[2..], b.line_no)?;
    Ok((key, lit))
}

fn parse_literal(toks: &[Tok], line: usize) -> PResult<Literal> {
    match toks.first() {
        Some(Tok::Number(n)) => Ok(Literal::Number(
            Rational::parse_decimal(n).ok_or_else(|| err(line, "bad number"))?,
        )),
        Some(Tok::Money(n)) => Ok(Literal::Money(
            Rational::parse_decimal(n).ok_or_else(|| err(line, "bad money"))?,
        )),
        Some(Tok::Bytes(b)) => Ok(Literal::Bytes(*b)),
        Some(Tok::Duration(d)) => Ok(Literal::Duration(*d)),
        Some(Tok::Str(s)) => Ok(Literal::Str(s.clone())),
        Some(Tok::Ident(s)) if s == "true" => Ok(Literal::Bool(true)),
        Some(Tok::Ident(s)) if s == "false" => Ok(Literal::Bool(false)),
        Some(Tok::Ident(s)) => Ok(Literal::Ident(s.clone())),
        Some(Tok::LBracket) => {
            let mut items = Vec::new();
            for t in &toks[1..] {
                match t {
                    Tok::RBracket => break,
                    Tok::Comma => {}
                    other => items.push(parse_literal(std::slice::from_ref(other), line)?),
                }
            }
            Ok(Literal::List(items))
        }
        _ => Err(err(line, "expected a literal value")),
    }
}

/// Parse a comparison predicate `lhs <op> rhs`.
fn parse_predicate(toks: &[Tok], line: usize) -> PResult<Predicate> {
    let split = toks
        .iter()
        .position(|t| matches!(t, Tok::Cmp(_)))
        .ok_or_else(|| err(line, "expected a comparison operator"))?;
    let op = match &toks[split] {
        Tok::Cmp(o) => *o,
        _ => unreachable!(),
    };
    let lhs = parse_expr(&toks[..split], line)?;
    let rhs = parse_expr(&toks[split + 1..], line)?;
    Ok(Predicate { lhs, op, rhs })
}

/// Parse an additive arithmetic expression (left-associative, `* /` bind
/// tighter than `+ -`).
fn parse_expr(toks: &[Tok], line: usize) -> PResult<Expr> {
    let mut i = 0;
    let e = parse_add(toks, &mut i, line)?;
    if i != toks.len() {
        return Err(err(line, "trailing tokens in expression"));
    }
    Ok(e)
}

fn parse_add(toks: &[Tok], i: &mut usize, line: usize) -> PResult<Expr> {
    let mut left = parse_mul(toks, i, line)?;
    while let Some(t) = toks.get(*i) {
        let op = match t {
            Tok::Plus => BinOp::Add,
            Tok::Minus => BinOp::Sub,
            _ => break,
        };
        *i += 1;
        let right = parse_mul(toks, i, line)?;
        left = Expr::Bin(op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_mul(toks: &[Tok], i: &mut usize, line: usize) -> PResult<Expr> {
    let mut left = parse_primary(toks, i, line)?;
    while let Some(t) = toks.get(*i) {
        let op = match t {
            Tok::Star => BinOp::Mul,
            Tok::Slash => BinOp::Div,
            _ => break,
        };
        *i += 1;
        let right = parse_primary(toks, i, line)?;
        left = Expr::Bin(op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_primary(toks: &[Tok], i: &mut usize, line: usize) -> PResult<Expr> {
    // Unary minus: negate a numeric literal, or `0 - expr` otherwise.
    if toks.get(*i) == Some(&Tok::Minus) {
        *i += 1;
        let inner = parse_primary(toks, i, line)?;
        return Ok(match inner {
            Expr::Lit(Literal::Number(r)) => Expr::Lit(Literal::Number(Rational::new(-r.num, r.den))),
            Expr::Lit(Literal::Money(r)) => Expr::Lit(Literal::Money(Rational::new(-r.num, r.den))),
            other => Expr::Bin(
                BinOp::Sub,
                Box::new(Expr::Lit(Literal::Number(Rational::int(0)))),
                Box::new(other),
            ),
        });
    }
    let t = toks.get(*i).ok_or_else(|| err(line, "expected expression"))?;
    *i += 1;
    Ok(match t {
        Tok::Ident(s) => {
            // Reassemble a dotted name like `trust.reviewed`.
            let mut name = s.clone();
            while toks.get(*i) == Some(&Tok::Dot) {
                let part = match toks.get(*i + 1) {
                    Some(Tok::Ident(p)) => p.clone(),
                    _ => return Err(err(line, "expected identifier after '.'")),
                };
                name.push('.');
                name.push_str(&part);
                *i += 2;
            }
            Expr::Var(name)
        }
        Tok::Number(n) => Expr::Lit(Literal::Number(
            Rational::parse_decimal(n).ok_or_else(|| err(line, "bad number"))?,
        )),
        Tok::Money(n) => Expr::Lit(Literal::Money(
            Rational::parse_decimal(n).ok_or_else(|| err(line, "bad money"))?,
        )),
        Tok::Bytes(b) => Expr::Lit(Literal::Bytes(*b)),
        Tok::Duration(d) => Expr::Lit(Literal::Duration(*d)),
        _ => return Err(err(line, "unexpected token in expression")),
    })
}
