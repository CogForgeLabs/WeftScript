//! JSON bridge for runtime values — used to pass data to and from child
//! processes in the multiprocessing builtins (`mp_map` / `proc_input`).

use crate::value::Value;
use serde_json::Value as J;
use std::collections::BTreeMap;

/// Convert a runtime value into JSON.
pub fn to_json(v: &Value) -> J {
    match v {
        Value::Nil => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Num(n) => {
            if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.0e15 {
                J::from(*n as i64)
            } else {
                serde_json::Number::from_f64(*n).map(J::Number).unwrap_or(J::Null)
            }
        }
        Value::Str(s) => J::String((**s).clone()),
        Value::List(l) => J::Array(l.iter().map(to_json).collect()),
        Value::Map(m) => J::Object(m.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
        // Streams aren't serializable as-is (could be infinite); drain to array.
        Value::Stream(s) => match s.drain() {
            Ok(items) => J::Array(items.iter().map(to_json).collect()),
            Err(_) => J::Null,
        },
    }
}

/// Convert JSON into a runtime value.
pub fn from_json(j: &J) -> Value {
    match j {
        J::Null => Value::Nil,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => Value::Num(n.as_f64().unwrap_or(0.0)),
        J::String(s) => Value::str(s.clone()),
        J::Array(a) => Value::list(a.iter().map(from_json).collect()),
        J::Object(o) => {
            let mut m = BTreeMap::new();
            for (k, v) in o {
                m.insert(k.clone(), from_json(v));
            }
            Value::map(m)
        }
    }
}

/// Serialize a value to a compact JSON string.
pub fn encode(v: &Value) -> String {
    to_json(v).to_string()
}

/// Parse a JSON string into a value (errors map to a message).
pub fn decode(s: &str) -> Result<Value, String> {
    let j: J = serde_json::from_str(s).map_err(|e| format!("invalid JSON: {e}"))?;
    Ok(from_json(&j))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_values() {
        let v = Value::list(vec![
            Value::Num(3.0),
            Value::str("hi"),
            Value::Bool(true),
            Value::Nil,
        ]);
        let s = encode(&v);
        assert_eq!(decode(&s).unwrap(), v);
    }

    #[test]
    fn round_trips_maps_and_floats() {
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), Value::Num(1.5));
        m.insert("b".to_string(), Value::list(vec![Value::Num(1.0), Value::Num(2.0)]));
        let v = Value::map(m);
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }
}
