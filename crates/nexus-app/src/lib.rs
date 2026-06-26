//! # nexus-app
//!
//! Runs **mixed** programs: a single file that declares the intent graph
//! (`thing`/`intent`/`constraint`/`guarantee`/…) *and* contains executable code
//! (`fn`/`main`/statements). The declarative half is exposed to the executable
//! half through host builtins, so running code can:
//!
//! * `validate(record, "Thing")` — structurally check data against a schema;
//! * `prove("Guarantee")` — formally prove a safety invariant via SMT;
//! * `check(record, "Constraint")` — evaluate a declared constraint at runtime;
//! * `things()` / `fields("Thing")` — introspect the declared schema.
//!
//! This is the bridge between Nexus's verifiable declarative core and its
//! general-purpose executable layer.

use nexus_core::{CmpOp, Expr, Literal, Node, Predicate, Rational};
use nexus_dsl::{build_blocks, parse_blocks, Program, DECL_KEYWORDS};
use nexus_exec::{app_from_blocks, HostFns, Interp, Value};
use std::collections::BTreeMap;

/// Run a mixed declarative+executable program, returning its printed output.
pub fn run_mixed(src: &str) -> Result<Vec<String>, String> {
    let blocks = build_blocks(src).map_err(|e| format!("line {}: {}", e.line, e.msg))?;
    // Declarative half.
    let decl_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| b.head().is_some_and(|h| DECL_KEYWORDS.contains(&h)))
        .cloned()
        .collect();
    let program = parse_blocks(&decl_blocks).map_err(|e| e.to_string())?;
    // Executable half (skips declarative blocks).
    let app = app_from_blocks(&blocks).map_err(|e| e.to_string())?;
    let host = DeclHost { program };
    Interp::run_with_host(&app, Box::new(host))
}

/// The declarative context exposed to executable code as host builtins.
struct DeclHost {
    program: Program,
}

impl DeclHost {
    fn thing<'a>(&'a self, name: &str) -> Option<&'a nexus_core::Thing> {
        self.program.nodes.iter().find_map(|n| match n {
            Node::Thing(t) if t.name == name => Some(t),
            _ => None,
        })
    }

    fn constraint<'a>(&'a self, name: &str) -> Option<&'a nexus_core::Constraint> {
        self.program.nodes.iter().find_map(|n| match n {
            Node::Constraint(c) if c.name == name => Some(c),
            _ => None,
        })
    }
}

impl HostFns for DeclHost {
    fn call(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        match name {
            "things" => Some(Ok(Value::list(
                self.program
                    .nodes
                    .iter()
                    .filter_map(|n| match n {
                        Node::Thing(t) => Some(Value::str(t.name.clone())),
                        _ => None,
                    })
                    .collect(),
            ))),
            "fields" => Some(host_fields(self, args)),
            "validate" => Some(host_validate(self, args)),
            "check" => Some(host_check(self, args)),
            "prove" => Some(host_prove(self, args)),
            _ => None,
        }
    }
}

fn as_map(v: &Value) -> Result<&BTreeMap<String, Value>, String> {
    match v {
        Value::Map(m) => Ok(m),
        _ => Err(format!("expected a record (map), got {}", v.type_name())),
    }
}

fn host_fields(h: &DeclHost, args: &[Value]) -> Result<Value, String> {
    let name = args.first().and_then(|v| v.as_str().ok()).ok_or("fields(name): name required")?;
    let t = h.thing(name).ok_or_else(|| format!("unknown thing `{}`", name))?;
    Ok(Value::list(t.fields.iter().map(|f| Value::str(f.name.clone())).collect()))
}

fn host_validate(h: &DeclHost, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("validate(record, \"Thing\") takes 2 arguments".into());
    }
    let rec = as_map(&args[0])?;
    let tname = args[1].as_str()?;
    let t = h.thing(tname).ok_or_else(|| format!("unknown thing `{}`", tname))?;
    for f in &t.fields {
        match rec.get(&f.name) {
            None => return Ok(Value::Bool(false)),
            Some(v) => {
                if !type_matches(&f.ty, v) {
                    return Ok(Value::Bool(false));
                }
            }
        }
    }
    Ok(Value::Bool(true))
}

fn type_matches(ty: &nexus_core::Ty, v: &Value) -> bool {
    use nexus_core::Ty;
    match ty {
        Ty::String | Ty::Uuid | Ty::Timestamp => matches!(v, Value::Str(_)),
        Ty::Decimal => matches!(v, Value::Num(_)),
        Ty::Bool => matches!(v, Value::Bool(_)),
        Ty::List(_) => matches!(v, Value::List(_)),
        Ty::Named(_) => true, // nested records validated separately
    }
}

fn host_check(h: &DeclHost, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("check(record, \"Constraint\") takes 2 arguments".into());
    }
    let rec = as_map(&args[0])?;
    let cname = args[1].as_str()?;
    let c = h.constraint(cname).ok_or_else(|| format!("unknown constraint `{}`", cname))?;
    for p in &c.predicates {
        if !eval_pred(p, rec)? {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn host_prove(h: &DeclHost, args: &[Value]) -> Result<Value, String> {
    let gname = args.first().and_then(|v| v.as_str().ok()).ok_or("prove(\"Guarantee\") requires a name")?;
    let g = h
        .program
        .guarantees
        .iter()
        .find(|g| g.name == gname)
        .ok_or_else(|| format!("unknown guarantee `{}`", gname))?;
    Ok(Value::Bool(nexus_verify::prove(&g.assumptions, &g.ensure).is_proven()))
}

/// Evaluate a declared constraint predicate against a runtime record.
fn eval_pred(p: &Predicate, rec: &BTreeMap<String, Value>) -> Result<bool, String> {
    let l = eval_expr(&p.lhs, rec)?;
    let r = eval_expr(&p.rhs, rec)?;
    Ok(match p.op {
        CmpOp::Lt => l < r,
        CmpOp::Le => l <= r,
        CmpOp::Gt => l > r,
        CmpOp::Ge => l >= r,
        CmpOp::Eq => (l - r).abs() < 1e-9,
        CmpOp::Ne => (l - r).abs() >= 1e-9,
    })
}

fn eval_expr(e: &Expr, rec: &BTreeMap<String, Value>) -> Result<f64, String> {
    match e {
        Expr::Var(name) => match rec.get(name) {
            Some(Value::Num(n)) => Ok(*n),
            Some(Value::Bool(b)) => Ok(if *b { 1.0 } else { 0.0 }),
            Some(other) => Err(format!("constraint var `{}` is {}, not numeric", name, other.type_name())),
            None => Err(format!("constraint references `{}`, missing from record", name)),
        },
        Expr::Lit(l) => Ok(lit_num(l)),
        Expr::Bin(op, a, b) => {
            let (x, y) = (eval_expr(a, rec)?, eval_expr(b, rec)?);
            use nexus_core::BinOp::*;
            Ok(match op {
                Add => x + y,
                Sub => x - y,
                Mul => x * y,
                Div => x / y,
            })
        }
    }
}

fn lit_num(l: &Literal) -> f64 {
    match l {
        Literal::Number(r) | Literal::Money(r) => rat(*r),
        Literal::Bytes(b) => *b as f64,
        Literal::Duration(d) => *d as f64,
        Literal::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

fn rat(r: Rational) -> f64 {
    r.num as f64 / r.den as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIXED: &str = "\
project Bank

thing Account
    id: UUID
    balance: Decimal

constraint Solvent
    balance >= 0

guarantee NoOverdraft
    assume balance == start - amount
    assume start - amount >= 0
    ensure balance >= 0

fn make(id, bal)
    return {\"id\": id, \"balance\": bal}

acct = make(\"a1\", 100)
print validate(acct, \"Account\")
print check(acct, \"Solvent\")
print check(make(\"a2\", -5), \"Solvent\")
print prove(\"NoOverdraft\")
print things()
print fields(\"Account\")
";

    #[test]
    fn mixed_declarative_executable() {
        let out = run_mixed(MIXED).unwrap();
        assert_eq!(out[0], "true"); // validate ok
        assert_eq!(out[1], "true"); // solvent (100 >= 0)
        assert_eq!(out[2], "false"); // -5 not solvent
        assert_eq!(out[3], "true"); // guarantee proven
        assert_eq!(out[4], "[Account]");
        assert_eq!(out[5], "[id, balance]");
    }

    #[test]
    fn validate_rejects_bad_record() {
        let out = run_mixed(
            "thing P\n    id: UUID\n    n: Decimal\nprint validate({\"id\": \"x\"}, \"P\")\n",
        )
        .unwrap();
        assert_eq!(out[0], "false"); // missing field n
    }

    #[test]
    fn pure_executable_still_runs() {
        // No declarations -> host builtins simply never fire.
        let out = run_mixed("print 2 + 3\n").unwrap();
        assert_eq!(out, vec!["5"]);
    }
}
