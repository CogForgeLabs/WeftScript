//! Dynamic runtime values for the executable layer.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

/// One message on a generator's stream channel.
#[derive(Debug)]
pub enum StreamMsg {
    Item(Value),
    Err(String),
}

/// The consumer end of a lazy generator. Items are produced on a worker thread
/// and delivered through a bounded channel, so memory stays flat and an
/// infinite generator is fine as long as you only `take` what you need.
#[derive(Debug)]
pub struct StreamHandle {
    rx: Mutex<Receiver<StreamMsg>>,
}

impl StreamHandle {
    pub fn new(rx: Receiver<StreamMsg>) -> StreamHandle {
        StreamHandle { rx: Mutex::new(rx) }
    }
    /// Pull the next item, blocking until one arrives or the stream ends.
    /// `None` = exhausted; `Some(Err)` = the generator raised an error.
    pub fn next(&self) -> Option<Result<Value, String>> {
        match self.rx.lock().unwrap().recv() {
            Ok(StreamMsg::Item(v)) => Some(Ok(v)),
            Ok(StreamMsg::Err(e)) => Some(Err(e)),
            Err(_) => None,
        }
    }
    /// Drain every remaining item into a vec (an error short-circuits).
    pub fn drain(&self) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        while let Some(item) = self.next() {
            out.push(item?);
        }
        Ok(out)
    }
}

/// A runtime value. Numbers are f64 (apps need division/roots/pow); the
/// declarative/verification layer keeps using exact rationals separately.
///
/// Reference-counted payloads use [`Arc`] (not `Rc`) so a `Value` is `Send +
/// Sync`. That is what lets the parallel builtins (`pmap`, `parallel`) move
/// values across thread boundaries safely.
#[derive(Clone, Debug)]
pub enum Value {
    Num(f64),
    Str(Arc<String>),
    Bool(bool),
    List(Arc<Vec<Value>>),
    /// An ordered string-keyed map (stable iteration for deterministic output).
    Map(Arc<BTreeMap<String, Value>>),
    /// A lazy generator stream (single-consumption).
    Stream(Arc<StreamHandle>),
    Nil,
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(Arc::new(s.into()))
    }
    pub fn list(v: Vec<Value>) -> Value {
        Value::List(Arc::new(v))
    }
    pub fn map(m: BTreeMap<String, Value>) -> Value {
        Value::Map(Arc::new(m))
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Stream(_) => true,
            Value::Nil => false,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "num",
            Value::Str(_) => "str",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Stream(_) => "stream",
            Value::Nil => "nil",
        }
    }

    pub fn as_num(&self) -> Result<f64, String> {
        match self {
            Value::Num(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            other => Err(format!("expected number, got {}", other.type_name())),
        }
    }

    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            Value::Str(s) => Ok(s),
            other => Err(format!("expected string, got {}", other.type_name())),
        }
    }
}

/// Structural equality used by `==`.
impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            // Streams compare by identity (they have no stable contents).
            (Value::Stream(a), Value::Stream(b)) => Arc::ptr_eq(a, b),
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }
}

/// Human-readable rendering (what `print` emits). Integers print without a
/// trailing `.0` so output matches conventional expectations.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Num(n) => {
                if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            // Printing a stream materializes it (drains lazily through the
            // bounded channel) and renders the collected items as a list. Use
            // `take(stream, n)` for infinite generators.
            Value::Stream(s) => match s.drain() {
                Ok(items) => {
                    write!(f, "[")?;
                    for (i, v) in items.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", v)?;
                    }
                    write!(f, "]")
                }
                Err(_) => write!(f, "<stream error>"),
            },
            Value::Nil => write!(f, "nil"),
        }
    }
}
