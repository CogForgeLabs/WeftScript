//! The built-in standard library: operators, indexing, iteration, and a broad
//! set of math / string / list / map functions. This breadth is what makes the
//! executable layer general-purpose rather than tied to one domain.

use crate::ast::BinOp;
use crate::value::Value;
use nexus_core::trace::{self, Level};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// The process-wide capability sandbox, configured from the environment
/// (`NEXUS_ALLOW_CMDS` / `NEXUS_DENY_CMDS` / `NEXUS_ALLOW_HOSTS` /
/// `NEXUS_DENY_HOSTS`). Auto-applied to every shell and network builtin, with a
/// built-in denylist of destructive commands that holds even with no config.
fn sandbox() -> &'static nexus_secure::Sandbox {
    static S: OnceLock<nexus_secure::Sandbox> = OnceLock::new();
    S.get_or_init(nexus_secure::Sandbox::from_env)
}

/// Trace which hardware backend the dispatcher selected for a vector op.
fn trace_dispatch(op: &str, n: usize) {
    let (dev, backend) = nexus_accel::route(n);
    trace::record(
        Level::Debug,
        "exec.accel",
        None,
        format!("dispatch {}", op),
        &[("n", &n.to_string()), ("device", &format!("{:?}", dev)), ("backend", backend.as_str())],
    );
}

type ER<T> = Result<T, String>;

/// Builtins that are pure computations: no I/O, no shell, no network, no
/// clock, deterministic for a given input. The auto-parallelizer only fans a
/// comprehension across threads when every called name is a user function it
/// can analyze or a member of this list — an unknown name (e.g. a host-provided
/// builtin like `validate`, which cannot cross threads) blocks parallelization.
pub const PURE_BUILTINS: &[&str] = &[
    // math
    "abs", "floor", "ceil", "sqrt", "sin", "cos", "tan", "log", "exp", "pi", "int", "pow", "round",
    "min", "max", "sum", "avg", "mean", "inf",
    // generic
    "len", "str", "num", "bool", "type",
    // strings
    "upper", "lower", "trim", "split", "join", "replace", "contains", "starts_with", "ends_with",
    "substr", "repeat", "ord", "chr", "find", "index_of", "rjust", "ljust", "lines",
    // lists
    "range", "push", "sort", "sort_desc", "reverse", "unique", "slice", "collect", "take", "first",
    "last", "count", "zip", "any", "all", "enumerate",
    // maps
    "map", "set", "get", "has", "keys", "values", "items", "del",
    // JSON (pure transforms)
    "json_encode", "json_decode",
    // security/privacy (pure text transforms)
    "redact", "contains_pii", "pii_kinds", "anonymize", "sandbox_ok",
    // vectors / hardware introspection
    "vadd", "vsub", "vmul", "vscale", "vsum", "vdot", "vmean", "vmax", "vmin", "accel_backend",
    "cores", "hardware",
];

/// Evaluate a binary operator over two values.
pub fn binary(op: BinOp, a: &Value, b: &Value) -> ER<Value> {
    use BinOp::*;
    match op {
        Add => match (a, b) {
            (Value::Num(x), Value::Num(y)) => Ok(Value::Num(x + y)),
            (Value::List(x), Value::List(y)) => {
                let mut v = (**x).clone();
                v.extend((**y).clone());
                Ok(Value::list(v))
            }
            // String concatenation if either side is a string.
            (Value::Str(_), _) | (_, Value::Str(_)) => {
                Ok(Value::str(format!("{}{}", a, b)))
            }
            _ => Err(format!("cannot add {} and {}", a.type_name(), b.type_name())),
        },
        Sub => Ok(Value::Num(a.as_num()? - b.as_num()?)),
        Mul => match (a, b) {
            // string * n repeats.
            (Value::Str(s), Value::Num(n)) | (Value::Num(n), Value::Str(s)) => {
                Ok(Value::str(s.repeat((*n).max(0.0) as usize)))
            }
            _ => Ok(Value::Num(a.as_num()? * b.as_num()?)),
        },
        Div => {
            let d = b.as_num()?;
            if d == 0.0 {
                return Err("division by zero".into());
            }
            Ok(Value::Num(a.as_num()? / d))
        }
        Mod => {
            let d = b.as_num()?;
            if d == 0.0 {
                return Err("modulo by zero".into());
            }
            Ok(Value::Num(a.as_num()? % d))
        }
        Eq => Ok(Value::Bool(a == b)),
        Ne => Ok(Value::Bool(a != b)),
        Lt | Le | Gt | Ge => {
            let ord = compare(a, b)?;
            Ok(Value::Bool(match op {
                Lt => ord == Ordering::Less,
                Le => ord != Ordering::Greater,
                Gt => ord == Ordering::Greater,
                Ge => ord != Ordering::Less,
                _ => unreachable!(),
            }))
        }
        And | Or => unreachable!("short-circuited in interpreter"),
    }
}

fn compare(a: &Value, b: &Value) -> ER<Ordering> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y).ok_or_else(|| "NaN comparison".into()),
        (Value::Str(x), Value::Str(y)) => Ok(x.cmp(y)),
        _ => Err(format!("cannot order {} and {}", a.type_name(), b.type_name())),
    }
}

pub fn index_get(coll: &Value, idx: &Value) -> ER<Value> {
    match coll {
        Value::List(l) => {
            let i = idx.as_num()? as i64;
            let i = if i < 0 { l.len() as i64 + i } else { i };
            l.get(i as usize)
                .cloned()
                .ok_or_else(|| format!("list index {} out of range", i))
        }
        Value::Str(s) => {
            let i = idx.as_num()? as usize;
            s.chars().nth(i).map(|c| Value::str(c.to_string())).ok_or_else(|| "string index out of range".into())
        }
        Value::Map(m) => {
            let k = idx.as_str()?;
            m.get(k).cloned().ok_or_else(|| format!("missing key `{}`", k))
        }
        _ => Err(format!("cannot index {}", coll.type_name())),
    }
}

pub fn index_set(coll: &Value, idx: &Value, v: Value) -> ER<Value> {
    let mut out = coll.clone();
    index_set_mut(&mut out, idx, v)?;
    Ok(out)
}

/// Assign `coll[idx] = v` in place. When the collection is uniquely owned this
/// mutates the existing allocation (no copy); when shared it copies once
/// (copy-on-write via [`Arc::make_mut`]). This is what makes the ubiquitous
/// `counts[k] = counts[k] + 1` accumulation pattern O(1) per step instead of
/// O(collection size).
pub fn index_set_mut(coll: &mut Value, idx: &Value, v: Value) -> ER<()> {
    match coll {
        Value::List(l) => {
            let i = idx.as_num()? as i64;
            let len = l.len() as i64;
            let i = if i < 0 { len + i } else { i };
            if i < 0 || i >= len {
                return Err(format!("list index {} out of range for assignment", i));
            }
            Arc::make_mut(l)[i as usize] = v;
            Ok(())
        }
        Value::Map(m) => {
            let k = idx.as_str()?.to_string();
            Arc::make_mut(m).insert(k, v);
            Ok(())
        }
        Value::Nil => {
            // Allow building a map from nil via string-key assignment.
            let mut nm = BTreeMap::new();
            nm.insert(idx.as_str()?.to_string(), v);
            *coll = Value::map(nm);
            Ok(())
        }
        _ => Err(format!("cannot index-assign {}", coll.type_name())),
    }
}

/// Append to a list in place (unique) or via one copy (shared). Nil becomes a
/// fresh single-element list, matching `push`'s copying behavior.
pub fn push_mut(coll: &mut Value, v: Value) -> ER<()> {
    match coll {
        Value::List(l) => {
            Arc::make_mut(l).push(v);
            Ok(())
        }
        Value::Nil => {
            *coll = Value::list(vec![v]);
            Ok(())
        }
        other => Err(format!("push: expected list, got {}", other.type_name())),
    }
}

/// Recursively set `container[i0][i1]... = value`, mutating in place along the
/// path (copy-on-write only where a node is shared). Lookup failures surface
/// before any mutation happens.
pub fn set_path_mut(container: &mut Value, indices: &[Value], value: Value) -> ER<()> {
    if indices.len() == 1 {
        return index_set_mut(container, &indices[0], value);
    }
    match container {
        Value::List(l) => {
            let i = indices[0].as_num()? as i64;
            let len = l.len() as i64;
            let i = if i < 0 { len + i } else { i };
            if i < 0 || i >= len {
                return Err(format!("list index {} out of range", i));
            }
            set_path_mut(&mut Arc::make_mut(l)[i as usize], &indices[1..], value)
        }
        Value::Map(m) => {
            let k = indices[0].as_str()?;
            if !m.contains_key(k) {
                return Err(format!("missing key `{}`", k));
            }
            let k = k.to_string();
            set_path_mut(Arc::make_mut(m).get_mut(&k).expect("checked above"), &indices[1..], value)
        }
        other => Err(format!("cannot index {}", other.type_name())),
    }
}

pub fn iterate(seq: &Value) -> ER<Vec<Value>> {
    Ok(iter_vals(seq)?.into_owned())
}

/// Iterate a sequence without copying when possible: lists lend their backing
/// slice; strings/maps/streams materialize.
pub fn iter_vals(seq: &Value) -> ER<std::borrow::Cow<'_, [Value]>> {
    use std::borrow::Cow;
    match seq {
        Value::List(l) => Ok(Cow::Borrowed(&l[..])),
        Value::Str(s) => Ok(Cow::Owned(s.chars().map(|c| Value::str(c.to_string())).collect())),
        Value::Map(m) => Ok(Cow::Owned(m.keys().map(|k| Value::str(k.clone())).collect())),
        // Draining a stream materializes it (lazily, through its bounded channel).
        Value::Stream(s) => Ok(Cow::Owned(s.drain()?)),
        _ => Err(format!("cannot iterate {}", seq.type_name())),
    }
}

fn arity(name: &str, args: &[Value], n: usize) -> ER<()> {
    if args.len() != n {
        Err(format!("`{}` expects {} argument(s), got {}", name, n, args.len()))
    } else {
        Ok(())
    }
}

/// Dispatch a builtin function call.
pub fn call(name: &str, args: &[Value]) -> ER<Value> {
    match name {
        // ---- math ----
        "abs" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.abs()))
        }
        "floor" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.floor()))
        }
        "ceil" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.ceil()))
        }
        "sqrt" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.sqrt()))
        }
        "sin" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.sin()))
        }
        "cos" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.cos()))
        }
        "tan" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.tan()))
        }
        "log" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.ln()))
        }
        "exp" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.exp()))
        }
        "pi" => {
            arity(name, args, 0)?;
            Ok(Value::Num(std::f64::consts::PI))
        }
        "int" => {
            arity(name, args, 1)?;
            Ok(Value::Num(args[0].as_num()?.trunc()))
        }
        "pow" => {
            arity(name, args, 2)?;
            Ok(Value::Num(args[0].as_num()?.powf(args[1].as_num()?)))
        }
        "round" => {
            if args.len() == 1 {
                Ok(Value::Num(args[0].as_num()?.round()))
            } else if args.len() == 2 {
                let x = args[0].as_num()?;
                let nd = args[1].as_num()? as i32;
                let f = 10f64.powi(nd);
                Ok(Value::Num((x * f).round() / f))
            } else {
                Err("`round` expects 1 or 2 arguments".into())
            }
        }
        "min" => fold_num(args, |a, b| a.min(b)),
        "max" => fold_num(args, |a, b| a.max(b)),
        "sum" => {
            arity(name, args, 1)?;
            let items = iter_vals(&args[0])?;
            let mut s = 0.0;
            for it in items.iter() {
                s += it.as_num()?;
            }
            Ok(Value::Num(s))
        }
        "avg" | "mean" => {
            arity(name, args, 1)?;
            let items = iter_vals(&args[0])?;
            if items.is_empty() {
                return Ok(Value::Num(0.0));
            }
            let mut s = 0.0;
            for it in items.iter() {
                s += it.as_num()?;
            }
            Ok(Value::Num(s / items.len() as f64))
        }

        // ---- generic ----
        "len" => {
            arity(name, args, 1)?;
            Ok(Value::Num(match &args[0] {
                Value::Str(s) => s.chars().count() as f64,
                Value::List(l) => l.len() as f64,
                Value::Map(m) => m.len() as f64,
                other => return Err(format!("len: unsupported {}", other.type_name())),
            }))
        }
        "str" => {
            arity(name, args, 1)?;
            Ok(Value::str(args[0].to_string()))
        }
        "num" => {
            arity(name, args, 1)?;
            match &args[0] {
                Value::Num(n) => Ok(Value::Num(*n)),
                Value::Str(s) => s.trim().parse::<f64>().map(Value::Num).map_err(|_| format!("cannot parse `{}` as number", s)),
                other => Err(format!("num: unsupported {}", other.type_name())),
            }
        }

        // ---- string ----
        "upper" => Ok(Value::str(arg_str(name, args)?.to_uppercase())),
        "lower" => Ok(Value::str(arg_str(name, args)?.to_lowercase())),
        "trim" => Ok(Value::str(arg_str(name, args)?.trim().to_string())),
        "split" => {
            arity(name, args, 2)?;
            let s = args[0].as_str()?;
            let sep = args[1].as_str()?;
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::str(c.to_string())).collect()
            } else {
                s.split(sep).map(|p| Value::str(p.to_string())).collect()
            };
            Ok(Value::list(parts))
        }
        "join" => {
            arity(name, args, 2)?;
            let items = iterate(&args[0])?;
            let sep = args[1].as_str()?;
            let parts: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            Ok(Value::str(parts.join(sep)))
        }
        "replace" => {
            arity(name, args, 3)?;
            Ok(Value::str(args[0].as_str()?.replace(args[1].as_str()?, args[2].as_str()?)))
        }
        "contains" => {
            arity(name, args, 2)?;
            match &args[0] {
                Value::Str(s) => Ok(Value::Bool(s.contains(args[1].as_str()?))),
                Value::List(l) => Ok(Value::Bool(l.iter().any(|x| x == &args[1]))),
                Value::Map(m) => Ok(Value::Bool(m.contains_key(args[1].as_str()?))),
                other => Err(format!("contains: unsupported {}", other.type_name())),
            }
        }
        "starts_with" => {
            arity(name, args, 2)?;
            Ok(Value::Bool(args[0].as_str()?.starts_with(args[1].as_str()?)))
        }
        "ends_with" => {
            arity(name, args, 2)?;
            Ok(Value::Bool(args[0].as_str()?.ends_with(args[1].as_str()?)))
        }
        "substr" => {
            arity(name, args, 3)?;
            let s: Vec<char> = args[0].as_str()?.chars().collect();
            let start = args[1].as_num()?.max(0.0) as usize;
            let count = args[2].as_num()?.max(0.0) as usize;
            let sub: String = s.iter().skip(start).take(count).collect();
            Ok(Value::str(sub))
        }
        "repeat" => {
            arity(name, args, 2)?;
            Ok(Value::str(args[0].as_str()?.repeat(args[1].as_num()?.max(0.0) as usize)))
        }
        "ord" => {
            arity(name, args, 1)?;
            let c = args[0].as_str()?.chars().next().ok_or("ord: empty string")?;
            Ok(Value::Num(c as u32 as f64))
        }
        "chr" => {
            arity(name, args, 1)?;
            let n = args[0].as_num()? as u32;
            let c = char::from_u32(n).ok_or_else(|| format!("chr: invalid code point {}", n))?;
            Ok(Value::str(c.to_string()))
        }
        "find" | "index_of" => {
            arity(name, args, 2)?;
            let s = args[0].as_str()?;
            let sub = args[1].as_str()?;
            Ok(Value::Num(match s.find(sub) {
                Some(b) => s[..b].chars().count() as f64,
                None => -1.0,
            }))
        }
        "rjust" => {
            arity(name, args, 2)?;
            let s = args[0].as_str()?;
            let w = args[1].as_num()? as usize;
            let pad = w.saturating_sub(s.chars().count());
            Ok(Value::str(format!("{}{}", " ".repeat(pad), s)))
        }
        "ljust" => {
            arity(name, args, 2)?;
            let s = args[0].as_str()?;
            let w = args[1].as_num()? as usize;
            let pad = w.saturating_sub(s.chars().count());
            Ok(Value::str(format!("{}{}", s, " ".repeat(pad))))
        }

        // ---- list ----
        "range" => builtin_range(args),
        "push" => {
            arity(name, args, 2)?;
            let mut l = match &args[0] {
                Value::List(l) => (**l).clone(),
                Value::Nil => Vec::new(),
                other => return Err(format!("push: expected list, got {}", other.type_name())),
            };
            l.push(args[1].clone());
            Ok(Value::list(l))
        }
        "sort" => sort_list(args, false),
        "sort_desc" => sort_list(args, true),
        "reverse" => {
            arity(name, args, 1)?;
            let mut l = iterate(&args[0])?;
            l.reverse();
            Ok(Value::list(l))
        }
        "unique" => {
            arity(name, args, 1)?;
            let items = iterate(&args[0])?;
            let mut out: Vec<Value> = Vec::new();
            for it in items {
                if !out.iter().any(|x| x == &it) {
                    out.push(it);
                }
            }
            Ok(Value::list(out))
        }
        "slice" => {
            arity(name, args, 3)?;
            let items = iterate(&args[0])?;
            let start = args[1].as_num()?.max(0.0) as usize;
            let end = (args[2].as_num()?.max(0.0) as usize).min(items.len());
            Ok(Value::list(items.into_iter().skip(start).take(end.saturating_sub(start)).collect()))
        }
        "collect" => {
            // Materialize a stream (or any iterable) into a list.
            arity(name, args, 1)?;
            Ok(Value::list(iterate(&args[0])?))
        }
        "take" => {
            // take(stream_or_list, n): pull at most n items. For an infinite
            // generator this is how you get a finite prefix without hanging.
            arity(name, args, 2)?;
            let n = args[1].as_num()?.max(0.0) as usize;
            match &args[0] {
                Value::Stream(s) => {
                    let mut out = Vec::with_capacity(n);
                    for _ in 0..n {
                        match s.next() {
                            Some(Ok(v)) => out.push(v),
                            Some(Err(e)) => return Err(e),
                            None => break,
                        }
                    }
                    Ok(Value::list(out))
                }
                _ => Ok(Value::list(iterate(&args[0])?.into_iter().take(n).collect())),
            }
        }
        "first" => {
            arity(name, args, 1)?;
            iterate(&args[0])?.into_iter().next().ok_or_else(|| "first: empty".into())
        }
        "last" => {
            arity(name, args, 1)?;
            iterate(&args[0])?.into_iter().last().ok_or_else(|| "last: empty".into())
        }
        "count" => {
            arity(name, args, 2)?;
            let items = iterate(&args[0])?;
            Ok(Value::Num(items.iter().filter(|x| **x == args[1]).count() as f64))
        }
        "zip" => {
            arity(name, args, 2)?;
            let a = iterate(&args[0])?;
            let b = iterate(&args[1])?;
            let pairs = a
                .into_iter()
                .zip(b)
                .map(|(x, y)| Value::list(vec![x, y]))
                .collect();
            Ok(Value::list(pairs))
        }
        "any" => {
            arity(name, args, 1)?;
            Ok(Value::Bool(iter_vals(&args[0])?.iter().any(|v| v.truthy())))
        }
        "all" => {
            arity(name, args, 1)?;
            Ok(Value::Bool(iter_vals(&args[0])?.iter().all(|v| v.truthy())))
        }
        "enumerate" => {
            arity(name, args, 1)?;
            let items = iter_vals(&args[0])?;
            let out: Vec<Value> = items
                .iter()
                .enumerate()
                .map(|(i, v)| Value::list(vec![Value::Num(i as f64), v.clone()]))
                .collect();
            Ok(Value::list(out))
        }
        "inf" => {
            arity(name, args, 0)?;
            Ok(Value::Num(f64::INFINITY))
        }
        "type" => {
            arity(name, args, 1)?;
            Ok(Value::str(args[0].type_name()))
        }
        "bool" => {
            arity(name, args, 1)?;
            Ok(Value::Bool(args[0].truthy()))
        }

        // ---- map ----
        "map" => {
            arity(name, args, 0)?;
            Ok(Value::Map(Arc::new(BTreeMap::new())))
        }
        "set" => {
            arity(name, args, 3)?;
            index_set(&args[0], &args[1], args[2].clone())
        }
        "get" => {
            if args.len() == 2 {
                index_get(&args[0], &args[1])
            } else if args.len() == 3 {
                Ok(index_get(&args[0], &args[1]).unwrap_or_else(|_| args[2].clone()))
            } else {
                Err("`get` expects 2 or 3 arguments".into())
            }
        }
        "has" => {
            arity(name, args, 2)?;
            match &args[0] {
                Value::Map(m) => Ok(Value::Bool(m.contains_key(args[1].as_str()?))),
                _ => Err("has: expected map".into()),
            }
        }
        "keys" => {
            arity(name, args, 1)?;
            match &args[0] {
                Value::Map(m) => Ok(Value::list(m.keys().map(|k| Value::str(k.clone())).collect())),
                _ => Err("keys: expected map".into()),
            }
        }
        "values" => {
            arity(name, args, 1)?;
            match &args[0] {
                Value::Map(m) => Ok(Value::list(m.values().cloned().collect())),
                _ => Err("values: expected map".into()),
            }
        }
        "items" => {
            arity(name, args, 1)?;
            match &args[0] {
                Value::Map(m) => Ok(Value::list(
                    m.iter()
                        .map(|(k, v)| Value::list(vec![Value::str(k.clone()), v.clone()]))
                        .collect(),
                )),
                _ => Err("items: expected map".into()),
            }
        }
        "del" => {
            arity(name, args, 2)?;
            match &args[0] {
                Value::Map(m) => {
                    let mut nm = m.clone();
                    Arc::make_mut(&mut nm).remove(args[1].as_str()?);
                    Ok(Value::Map(nm))
                }
                _ => Err("del: expected map".into()),
            }
        }

        // ---- JSON ----
        "json_encode" => {
            arity(name, args, 1)?;
            Ok(Value::str(crate::json::encode(&args[0])))
        }
        "json_decode" => {
            arity(name, args, 1)?;
            crate::json::decode(args[0].as_str()?)
        }

        // ---- files (same power class as `sh`, minus the shell) ----
        "read_file" => {
            arity(name, args, 1)?;
            std::fs::read_to_string(args[0].as_str()?)
                .map(Value::str)
                .map_err(|e| format!("read_file: {}: {}", args[0], e))
        }
        "write_file" => {
            arity(name, args, 2)?;
            std::fs::write(args[0].as_str()?, args[1].as_str()?)
                .map(|_| Value::Nil)
                .map_err(|e| format!("write_file: {}: {}", args[0], e))
        }
        "append_file" => {
            arity(name, args, 2)?;
            use std::io::Write;
            let (path, text) = (args[0].as_str()?, args[1].as_str()?);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| f.write_all(text.as_bytes()))
                .map(|_| Value::Nil)
                .map_err(|e| format!("append_file: {}: {}", path, e))
        }

        // ---- time ----
        "now_ms" => {
            arity(name, args, 0)?;
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0);
            Ok(Value::Num(ms))
        }

        // ---- text ----
        "lines" => {
            arity(name, args, 1)?;
            Ok(Value::list(
                args[0]
                    .as_str()?
                    .lines()
                    .map(|l| Value::str(l.trim_end_matches('\r')))
                    .collect(),
            ))
        }

        // ---- terminal / tool automation (auto-sandboxed) ----
        "sh" => {
            arity(name, args, 1)?;
            let cmd = args[0].as_str()?;
            sandbox().check_command(cmd)?;
            Ok(cmd_to_value(nexus_tool::run(cmd)))
        }
        "sh_timeout" => {
            arity(name, args, 2)?;
            let cmd = args[0].as_str()?;
            sandbox().check_command(cmd)?;
            let ms = args[1].as_num()?.max(0.0) as u64;
            Ok(cmd_to_value(nexus_tool::run_timeout(cmd, Some(ms))))
        }
        "which" => {
            arity(name, args, 1)?;
            match nexus_tool::which(args[0].as_str()?) {
                Some(p) => Ok(Value::str(p)),
                None => Ok(Value::Nil),
            }
        }
        "auto_fix" => {
            arity(name, args, 1)?;
            sandbox().check_command(args[0].as_str()?)?;
            let (winner, res) = nexus_tool::auto_fix(args[0].as_str()?);
            let mut m = match cmd_to_value(res) {
                Value::Map(m) => (*m).clone(),
                _ => BTreeMap::new(),
            };
            m.insert("cmd".into(), Value::str(winner));
            Ok(Value::map(m))
        }
        "tool_run" => {
            // tool_run(template, params_map) -> renders {key} then runs.
            arity(name, args, 2)?;
            let template = args[0].as_str()?;
            let params = match &args[1] {
                Value::Map(m) => m.iter().map(|(k, v)| (k.clone(), v.to_string())).collect(),
                _ => return Err("tool_run: second argument must be a map".into()),
            };
            let cmd = nexus_tool::render_template(template, &params);
            sandbox().check_command(&cmd)?;
            Ok(cmd_to_value(nexus_tool::run(&cmd)))
        }
        "psh" => {
            // Parallel shell: run a list of commands concurrently (each sandboxed).
            arity(name, args, 1)?;
            let cmds: Vec<String> = iterate(&args[0])?.iter().map(|v| v.to_string()).collect();
            for c in &cmds {
                sandbox().check_command(c)?;
            }
            let results = nexus_tool::run_parallel(&cmds);
            Ok(Value::list(results.into_iter().map(cmd_to_value).collect()))
        }

        // ---- security / privacy / anonymity ----
        "redact" => {
            arity(name, args, 1)?;
            Ok(Value::str(nexus_secure::redact(args[0].as_str()?)))
        }
        "contains_pii" => {
            arity(name, args, 1)?;
            Ok(Value::Bool(nexus_secure::contains_pii(args[0].as_str()?)))
        }
        "pii_kinds" => {
            arity(name, args, 1)?;
            Ok(Value::list(nexus_secure::pii_kinds(args[0].as_str()?).into_iter().map(Value::str).collect()))
        }
        "anonymize" => {
            arity(name, args, 1)?;
            Ok(Value::str(nexus_secure::anonymize(args[0].as_str()?)))
        }
        "anon_token" => {
            arity(name, args, 0)?;
            Ok(Value::str(nexus_secure::anon_token()))
        }
        "sandbox_ok" => {
            // sandbox_ok(cmd) -> bool: would the sandbox allow this command?
            arity(name, args, 1)?;
            Ok(Value::Bool(sandbox().check_command(args[0].as_str()?).is_ok()))
        }

        // ---- multiprocessing (separate OS processes) ----
        "proc_input" => {
            // Read this process's input payload (set by a parent `mp_map`).
            arity(name, args, 0)?;
            match std::env::var("NEXUS_PROC_INPUT") {
                Ok(s) if !s.is_empty() => crate::json::decode(&s),
                _ => Ok(Value::Nil),
            }
        }
        "mp_map" => {
            // mp_map(src, list): run `src` once per element in a SEPARATE
            // process, with the element exposed to the child as `input`.
            arity(name, args, 2)?;
            let src = args[0].as_str()?.to_string();
            let items = iterate(&args[1])?;
            multiprocess_map(&src, &items)
        }

        // ---- networking (real sockets, std::net; auto-sandboxed hosts) ----
        "tcp_request" => {
            arity(name, args, 2)?;
            sandbox().check_host(args[0].as_str()?)?;
            nexus_tool::net::tcp_request(args[0].as_str()?, args[1].as_str()?).map(Value::str)
        }
        "tcp_line" => {
            arity(name, args, 2)?;
            sandbox().check_host(args[0].as_str()?)?;
            nexus_tool::net::tcp_request_line(args[0].as_str()?, args[1].as_str()?).map(Value::str)
        }
        "http_get" => {
            arity(name, args, 1)?;
            sandbox().check_host(args[0].as_str()?)?;
            let r = nexus_tool::net::http_get(args[0].as_str()?)?;
            let mut m = BTreeMap::new();
            m.insert("status".into(), Value::Num(r.status as f64));
            m.insert("body".into(), Value::str(r.body));
            Ok(Value::map(m))
        }

        // ---- logging / tracing ----
        // Note: `log` is the natural logarithm (math); structured logging uses
        // `trace` to avoid the collision.
        "trace" => {
            // trace(msg) -> info; trace(level, msg) -> leveled. Returns nil.
            let (level, msg) = match args.len() {
                1 => (Level::Info, args[0].to_string()),
                2 => (Level::parse(args[0].as_str()?), args[1].to_string()),
                _ => return Err("`trace` expects (msg) or (level, msg)".into()),
            };
            trace::record(level, "app", None, msg, &[]);
            Ok(Value::Nil)
        }
        "trace_count" => {
            arity(name, args, 0)?;
            Ok(Value::Num(trace::count() as f64))
        }

        // ---- hardware-accelerated numeric vectors (auto-dispatched) ----
        "vadd" => {
            trace_dispatch("vadd", list_len(args));
            vec_binary(name, args, nexus_accel::add)
        }
        "vsub" => {
            trace_dispatch("vsub", list_len(args));
            vec_binary(name, args, nexus_accel::sub)
        }
        "vmul" => {
            trace_dispatch("vmul", list_len(args));
            vec_binary(name, args, nexus_accel::mul)
        }
        "vscale" => {
            arity(name, args, 2)?;
            let v = to_f64s(&args[0])?;
            Ok(from_f64s(nexus_accel::scale(&v, args[1].as_num()?)))
        }
        "vsum" => {
            arity(name, args, 1)?;
            Ok(Value::Num(nexus_accel::sum(&to_f64s(&args[0])?)))
        }
        "vdot" => {
            arity(name, args, 2)?;
            let (x, y) = (to_f64s(&args[0])?, to_f64s(&args[1])?);
            if x.len() != y.len() {
                return Err("vdot: length mismatch".into());
            }
            trace_dispatch("vdot", x.len());
            Ok(Value::Num(nexus_accel::dot(&x, &y)))
        }
        "vmean" => {
            arity(name, args, 1)?;
            Ok(Value::Num(nexus_accel::mean(&to_f64s(&args[0])?)))
        }
        "vmax" => {
            arity(name, args, 1)?;
            Ok(Value::Num(nexus_accel::max(&to_f64s(&args[0])?)))
        }
        "vmin" => {
            arity(name, args, 1)?;
            Ok(Value::Num(nexus_accel::min(&to_f64s(&args[0])?)))
        }
        // Which strategy the dispatcher picks for a workload of size n.
        "accel_backend" => {
            arity(name, args, 1)?;
            let (dev, backend) = nexus_accel::route(args[0].as_num()? as usize);
            Ok(Value::str(format!("{:?}/{}", dev, backend.as_str())))
        }
        "cores" => {
            arity(name, args, 0)?;
            Ok(Value::Num(nexus_accel::cores() as f64))
        }
        "hardware" => {
            arity(name, args, 0)?;
            let mut m = BTreeMap::new();
            m.insert("cores".into(), Value::Num(nexus_accel::cores() as f64));
            m.insert("simd_lanes".into(), Value::Num(nexus_accel::simd_lanes() as f64));
            let mut devs: Vec<Value> = nexus_accel::available_devices()
                .iter()
                .map(|d| Value::str(format!("{:?}", d)))
                .collect();
            match nexus_accel::gpu_name() {
                Some(g) => {
                    devs.push(Value::str(format!("Gpu({})", g)));
                    m.insert("gpu".into(), Value::str(g));
                }
                None => {
                    m.insert("gpu".into(), Value::str("none"));
                }
            }
            m.insert("devices".into(), Value::list(devs));
            Ok(Value::map(m))
        }

        _ => Err(format!("unknown function `{}`", name)),
    }
}

/// Locate the `nexus` binary for spawning child processes: `NEXUS_BIN` if set,
/// else the currently running executable.
fn nexus_binary() -> Result<std::path::PathBuf, String> {
    if let Ok(p) = std::env::var("NEXUS_BIN") {
        return Ok(std::path::PathBuf::from(p));
    }
    std::env::current_exe().map_err(|e| format!("cannot locate nexus binary: {e}"))
}

/// True multiprocessing: run `src` (prefixed so the child binds `input =
/// proc_input()`) once per item, each in its own OS process. Children run
/// concurrently in batches sized to the core count; each child receives its
/// element as JSON via the `NEXUS_PROC_INPUT` env var and we collect its
/// trimmed stdout. This is process-level parallelism — no shared interpreter,
/// no GIL-equivalent, full memory isolation.
fn multiprocess_map(src: &str, items: &[Value]) -> ER<Value> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let bin = nexus_binary()?;
    let child_src = format!("input = proc_input()\n{}", src);
    let batch = nexus_accel::cores().max(1);
    let mut out: Vec<Value> = Vec::with_capacity(items.len());

    nexus_core::trace::record(
        nexus_core::trace::Level::Debug,
        "exec.mp",
        None,
        "multiprocess map",
        &[("items", &items.len().to_string()), ("batch", &batch.to_string())],
    );

    for chunk in items.chunks(batch) {
        // Spawn every child in this batch, then collect — true parallelism.
        let mut kids = Vec::with_capacity(chunk.len());
        for item in chunk {
            let payload = crate::json::encode(item);
            let mut child = Command::new(&bin)
                .args(["app", "-"])
                .env("NEXUS_PROC_INPUT", payload)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("mp_map: spawn failed: {e}"))?;
            if let Some(mut sin) = child.stdin.take() {
                sin.write_all(child_src.as_bytes()).map_err(|e| format!("mp_map: write failed: {e}"))?;
            }
            kids.push(child);
        }
        for child in kids {
            let res = child.wait_with_output().map_err(|e| format!("mp_map: wait failed: {e}"))?;
            if !res.status.success() {
                let err = String::from_utf8_lossy(&res.stderr);
                return Err(format!("mp_map: child failed: {}", err.trim()));
            }
            let text = String::from_utf8_lossy(&res.stdout).trim().to_string();
            out.push(Value::str(text));
        }
    }
    Ok(Value::list(out))
}

/// Convert a tool command result into a Nexus map value.
fn cmd_to_value(r: nexus_tool::CmdResult) -> Value {
    let mut m = BTreeMap::new();
    m.insert("code".into(), Value::Num(r.code as f64));
    m.insert("stdout".into(), Value::str(r.stdout));
    m.insert("stderr".into(), Value::str(r.stderr));
    m.insert("ok".into(), Value::Bool(r.ok));
    m.insert("timed_out".into(), Value::Bool(r.timed_out));
    Value::map(m)
}

/// Length of the first list argument (for dispatch tracing); 0 if not a list.
fn list_len(args: &[Value]) -> usize {
    match args.first() {
        Some(Value::List(l)) => l.len(),
        _ => 0,
    }
}

/// Extract a `Vec<f64>` from a list value.
fn to_f64s(v: &Value) -> ER<Vec<f64>> {
    match v {
        Value::List(l) => l.iter().map(|x| x.as_num()).collect(),
        _ => Err(format!("expected a list of numbers, got {}", v.type_name())),
    }
}

fn from_f64s(v: Vec<f64>) -> Value {
    Value::list(v.into_iter().map(Value::Num).collect())
}

fn vec_binary(name: &str, args: &[Value], f: fn(&[f64], &[f64]) -> Vec<f64>) -> ER<Value> {
    arity(name, args, 2)?;
    let (a, b) = (to_f64s(&args[0])?, to_f64s(&args[1])?);
    if a.len() != b.len() {
        return Err(format!("{}: length mismatch ({} vs {})", name, a.len(), b.len()));
    }
    Ok(from_f64s(f(&a, &b)))
}

fn arg_str<'a>(name: &str, args: &'a [Value]) -> ER<&'a str> {
    arity(name, args, 1)?;
    args[0].as_str()
}

fn fold_num(args: &[Value], f: impl Fn(f64, f64) -> f64) -> ER<Value> {
    // Accept either varargs or a single list.
    let nums: Vec<f64> = if args.len() == 1 {
        if let Value::List(_) = &args[0] {
            iterate(&args[0])?.iter().map(|v| v.as_num()).collect::<ER<_>>()?
        } else {
            vec![args[0].as_num()?]
        }
    } else {
        args.iter().map(|v| v.as_num()).collect::<ER<_>>()?
    };
    let mut it = nums.into_iter();
    let first = it.next().ok_or("min/max: no arguments")?;
    Ok(Value::Num(it.fold(first, f)))
}

fn builtin_range(args: &[Value]) -> ER<Value> {
    let (start, end, step) = match args.len() {
        1 => (0.0, args[0].as_num()?, 1.0),
        2 => (args[0].as_num()?, args[1].as_num()?, 1.0),
        3 => (args[0].as_num()?, args[1].as_num()?, args[2].as_num()?),
        _ => return Err("`range` expects 1-3 arguments".into()),
    };
    if step == 0.0 {
        return Err("range step cannot be zero".into());
    }
    // Hard ceiling so a single `range` can't trigger a catastrophic allocation
    // before the interpreter's cumulative accounting gets a chance to react.
    const RANGE_MAX: f64 = 50_000_000.0;
    let span = ((end - start) / step).abs();
    if span > RANGE_MAX {
        return Err(format!("range too large ({} elements exceeds {} cap)", span as u64, RANGE_MAX as u64));
    }
    let mut out = Vec::new();
    let mut x = start;
    if step > 0.0 {
        while x < end {
            out.push(Value::Num(x));
            x += step;
        }
    } else {
        while x > end {
            out.push(Value::Num(x));
            x += step;
        }
    }
    Ok(Value::list(out))
}

fn sort_list(args: &[Value], desc: bool) -> ER<Value> {
    arity("sort", args, 1)?;
    let mut items = iterate(&args[0])?;
    let mut err = None;
    items.sort_by(|a, b| match compare(a, b) {
        Ok(o) => o,
        Err(e) => {
            err.get_or_insert(e);
            Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    if desc {
        items.reverse();
    }
    Ok(Value::list(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_strings() {
        assert_eq!(binary(BinOp::Add, &Value::Num(2.0), &Value::Num(3.0)).unwrap(), Value::Num(5.0));
        assert_eq!(
            binary(BinOp::Add, &Value::str("a"), &Value::Num(1.0)).unwrap(),
            Value::str("a1")
        );
    }

    #[test]
    fn list_and_map_ops() {
        let l = call("range", &[Value::Num(0.0), Value::Num(3.0)]).unwrap();
        assert_eq!(call("len", &[l.clone()]).unwrap(), Value::Num(3.0));
        let m = call("set", &[call("map", &[]).unwrap(), Value::str("k"), Value::Num(9.0)]).unwrap();
        assert_eq!(call("get", &[m, Value::str("k")]).unwrap(), Value::Num(9.0));
    }

    #[test]
    fn string_processing() {
        let parts = call("split", &[Value::str("a,b,c"), Value::str(",")]).unwrap();
        assert_eq!(call("len", &[parts]).unwrap(), Value::Num(3.0));
        assert_eq!(call("upper", &[Value::str("hi")]).unwrap(), Value::str("HI"));
    }
}
