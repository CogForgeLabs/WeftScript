//! DSL projection: render a graph [`Program`] back to canonical Nexus DSL text.
//!
//! This is the *target* projection of the Triple Graph Grammar. Together with
//! the parser (the reverse projection) it guarantees the graph and text views
//! stay in lock-step: parsing the printed text reproduces the exact same graph.

use nexus_core::*;
use nexus_dsl::Program;

/// Render a whole program to DSL text.
pub fn to_dsl(prog: &Program) -> String {
    let mut out = String::new();
    out.push_str(&format!("project {}\n", prog.project));
    for node in &prog.nodes {
        out.push('\n');
        print_node(&mut out, node);
    }
    for g in &prog.guarantees {
        out.push('\n');
        out.push_str(&format!("guarantee {}\n", g.name));
        for a in &g.assumptions {
            out.push_str(&format!("    assume {} {} {}\n", expr(&a.lhs), a.op.as_str(), expr(&a.rhs)));
        }
        out.push_str(&format!(
            "    ensure {} {} {}\n",
            expr(&g.ensure.lhs),
            g.ensure.op.as_str(),
            expr(&g.ensure.rhs)
        ));
    }
    if !prog.directives.is_empty() {
        out.push('\n');
        for d in &prog.directives {
            print_directive(&mut out, d);
        }
    }
    out
}

fn print_node(out: &mut String, node: &Node) {
    match node {
        Node::Policy(p) => print_policy(out, p),
        Node::Thing(t) => print_thing(out, t),
        Node::Capability(c) => print_capability(out, c),
        Node::Contract(c) => print_contract(out, c),
        Node::Constraint(c) => print_constraint(out, c),
        Node::Intent(i) => print_intent(out, i),
        Node::Event(e) => out.push_str(&format!("event {}\n", e.name)),
        Node::Agent(a) => print_agent(out, a),
        Node::Workflow(w) => print_workflow(out, w),
        Node::Directive(d) => print_directive(out, d),
    }
}

fn print_policy(out: &mut String, p: &Policy) {
    out.push_str(&format!("policy {}\n", p.name));
    for d in &p.directives {
        match d {
            SecurityDirective::Restrict { action, target } => {
                out.push_str(&format!("    restrict {} -> {}\n", action, target))
            }
            SecurityDirective::Encrypt(t) => out.push_str(&format!("    encrypt {}\n", t)),
            SecurityDirective::Audit(b) => out.push_str(&format!("    audit {}\n", b)),
            SecurityDirective::Trust(t) => out.push_str(&format!("    trust {}\n", t)),
            SecurityDirective::DefaultDeny(b) => out.push_str(&format!("    default_deny {}\n", b)),
        }
    }
}

fn print_thing(out: &mut String, t: &Thing) {
    match &t.implements {
        Some(i) => out.push_str(&format!("thing {} implements {}\n", t.name, i)),
        None => out.push_str(&format!("thing {}\n", t.name)),
    }
    for f in &t.fields {
        match &f.policy {
            Some(p) => out.push_str(&format!("    {}: {} policy {}\n", f.name, f.ty, p)),
            None => out.push_str(&format!("    {}: {}\n", f.name, f.ty)),
        }
    }
}

fn print_capability(out: &mut String, c: &Capability) {
    out.push_str(&format!("capability {}\n", c.name));
    out.push_str("    signature\n");
    out.push_str(&format!("        input: {}\n", named_types(&c.signature.inputs)));
    out.push_str(&format!("        output: {}\n", named_types(&c.signature.outputs)));
    out.push_str("    guarantee\n");
    for (k, v) in &c.guarantees {
        out.push_str(&format!("        {}: {}\n", k, literal(v)));
    }
}

fn print_contract(out: &mut String, c: &Contract) {
    out.push_str(&format!("contract {}\n", c.name));
    for (k, v) in &c.limits {
        out.push_str(&format!("    {}: {}\n", k, literal(v)));
    }
}

fn print_constraint(out: &mut String, c: &Constraint) {
    out.push_str(&format!("constraint {}\n", c.name));
    for p in &c.predicates {
        out.push_str(&format!(
            "    {} {} {}\n",
            expr(&p.lhs),
            p.op.as_str(),
            expr(&p.rhs)
        ));
    }
}

fn print_intent(out: &mut String, i: &Intent) {
    out.push_str(&format!("intent {}\n", i.name));
    if !i.inputs.is_empty() {
        out.push_str(&format!("    input: {}\n", named_types(&i.inputs)));
    }
    if !i.goals.is_empty() {
        out.push_str("    goal\n");
        for g in &i.goals {
            out.push_str(&format!("        {}\n", g));
        }
    }
    for c in &i.constraints {
        out.push_str(&format!("    constraint {}\n", c));
    }
}

fn print_agent(out: &mut String, a: &Agent) {
    out.push_str(&format!("agent {}\n", a.name));
    if !a.goals.is_empty() {
        out.push_str("    goal\n");
        for g in &a.goals {
            out.push_str(&format!("        {}\n", g));
        }
    }
    if let Some(m) = &a.metric {
        out.push_str(&format!("    metric {}\n", m.name));
        for (obj, t) in &m.objectives {
            let kw = match obj {
                Objective::Maximize => "maximize",
                Objective::Minimize => "minimize",
            };
            out.push_str(&format!("        {} {}\n", kw, t));
        }
    }
    if !a.tools.is_empty() {
        out.push_str(&format!("    tools: [{}]\n", a.tools.join(", ")));
    }
    for c in &a.constraints {
        out.push_str(&format!("    constraint {}\n", c));
    }
}

fn print_workflow(out: &mut String, w: &Workflow) {
    out.push_str(&format!("workflow {}\n", w.name));
    out.push_str(&format!("    trigger on {}\n", w.trigger));
    for s in &w.steps {
        out.push_str(&format!("    step {}\n", s.name));
        if let Some(i) = &s.intent {
            out.push_str(&format!("        intent {}\n", i));
        }
        if let Some(d) = s.deterministic {
            out.push_str(&format!("        deterministic {}\n", d));
        }
        if let Some(n) = &s.need {
            out.push_str(&format!("        need: {}\n", n));
        }
        if let Some(c) = &s.contract {
            out.push_str(&format!("        contract {}\n", c));
        }
        if let Some(a) = &s.agent {
            out.push_str(&format!("        agent {}\n", a));
        }
        if let Some(r) = &s.reasoning {
            out.push_str(&format!("        reasoning {}\n", r));
        }
        if !s.tools.is_empty() {
            out.push_str(&format!("        tools: [{}]\n", s.tools.join(", ")));
        }
        if let Some(fb) = &s.fallback {
            out.push_str("        fallback\n");
            if let Some(r) = fb.retry {
                out.push_str(&format!("            retry {}\n", r));
            }
            if let Some(b) = &fb.backoff {
                out.push_str(&format!("            backoff {}\n", b));
            }
            if let Some(f) = &fb.on_failure {
                out.push_str(&format!("            on_failure {}\n", f));
            }
        }
    }
}

fn print_directive(out: &mut String, d: &Directive) {
    match d {
        Directive::Schedule { target, mode } => {
            out.push_str(&format!("schedule {} {}\n", target, mode))
        }
        Directive::Observe(m) => out.push_str(&format!("observe {}\n", m)),
        Directive::Optimize(m) => out.push_str(&format!("optimize {}\n", m)),
        Directive::Secure(m) => out.push_str(&format!("secure {}\n", m)),
    }
}

fn named_types(items: &[(String, Ty)]) -> String {
    items
        .iter()
        .map(|(n, t)| format!("{}: {}", n, t))
        .collect::<Vec<_>>()
        .join(", ")
}

fn literal(l: &Literal) -> String {
    match l {
        Literal::Number(r) => rational_decimal(*r),
        Literal::Money(r) => format!("${}", rational_decimal(*r)),
        Literal::Bytes(b) => format!("{}B", b),
        Literal::Duration(d) => format!("{}ms", d),
        Literal::Str(s) => format!("\"{}\"", s),
        Literal::Bool(b) => b.to_string(),
        Literal::Ident(s) => s.clone(),
        Literal::List(items) => {
            let inner: Vec<String> = items.iter().map(literal).collect();
            format!("[{}]", inner.join(", "))
        }
    }
}

fn expr(e: &Expr) -> String {
    match e {
        Expr::Var(s) => s.clone(),
        Expr::Lit(l) => literal(l),
        Expr::Bin(op, a, b) => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
            };
            format!("{} {} {}", expr(a), o, expr(b))
        }
    }
}

/// Render a rational as an exact terminating decimal where possible.
fn rational_decimal(r: Rational) -> String {
    let neg = r.num < 0;
    let mut rem = r.num.abs();
    let den = r.den;
    let int = rem / den;
    rem %= den;
    let sign = if neg { "-" } else { "" };
    if rem == 0 {
        return format!("{}{}", sign, int);
    }
    let mut frac = String::new();
    let mut guard = 0;
    while rem != 0 && guard < 18 {
        rem *= 10;
        let d = rem / den;
        frac.push((b'0' + d as u8) as char);
        rem %= den;
        guard += 1;
    }
    format!("{}{}.{}", sign, int, frac)
}
