//! Model Context Protocol binding over JSON-RPC 2.0.
//!
//! MCP discovers, queries, and binds external interfaces dynamically at runtime
//! using JSON-RPC 2.0. This module implements the wire types and an in-process
//! dispatcher so the runtime can negotiate capabilities the same way it would
//! over a socket — fully serializable and testable.

use crate::registry::{Objective, Registry, Requirement, Trust};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: u64,
}

impl JsonRpcRequest {
    pub fn new(method: &str, params: Value, id: u64) -> Self {
        JsonRpcRequest { jsonrpc: "2.0".into(), method: method.into(), params, id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

impl JsonRpcResponse {
    fn ok(id: u64, result: Value) -> Self {
        JsonRpcResponse { jsonrpc: "2.0".into(), result: Some(result), error: None, id }
    }
    fn err(id: u64, code: i64, message: &str) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(JsonRpcError { code, message: message.into() }),
            id,
        }
    }
}

/// A JSON-RPC endpoint that serves capability discovery over a registry.
pub struct McpServer<'a> {
    registry: &'a Registry,
}

impl<'a> McpServer<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        McpServer { registry }
    }

    /// Handle one JSON-RPC request, dispatching on `method`.
    ///
    /// * `discover` — list all capability names.
    /// * `query` — list providers (names) for `{capability}`.
    /// * `bind` — resolve the best provider for `{capability, objective,
    ///   min_trust?, compliance?}`.
    pub fn handle(&self, req: &JsonRpcRequest) -> JsonRpcResponse {
        if req.jsonrpc != "2.0" {
            return JsonRpcResponse::err(req.id, -32600, "invalid JSON-RPC version");
        }
        match req.method.as_str() {
            "discover" => JsonRpcResponse::ok(req.id, json!({ "capabilities": self.registry.capabilities() })),
            "query" => {
                let cap = match req.params.get("capability").and_then(Value::as_str) {
                    Some(c) => c,
                    None => return JsonRpcResponse::err(req.id, -32602, "missing 'capability'"),
                };
                let names: Vec<&str> = self
                    .registry
                    .providers()
                    .iter()
                    .filter(|p| p.capability == cap)
                    .map(|p| p.name.as_str())
                    .collect();
                JsonRpcResponse::ok(req.id, json!({ "providers": names }))
            }
            "bind" => {
                let cap = match req.params.get("capability").and_then(Value::as_str) {
                    Some(c) => c,
                    None => return JsonRpcResponse::err(req.id, -32602, "missing 'capability'"),
                };
                let objective = match req.params.get("objective").and_then(Value::as_str) {
                    Some("cost") => Objective::Cost,
                    Some("latency") => Objective::Latency,
                    Some("trust") => Objective::Trust,
                    Some("reliability") => Objective::Reliability,
                    _ => Objective::Cost,
                };
                let mut r = Requirement::new(cap, objective);
                if let Some(t) = req.params.get("min_trust").and_then(Value::as_str) {
                    r = r.min_trust(match t {
                        "proven" => Trust::Proven,
                        "verified" => Trust::Verified,
                        "reviewed" => Trust::Reviewed,
                        _ => Trust::Unverified,
                    });
                }
                if let Some(cs) = req.params.get("compliance").and_then(Value::as_array) {
                    for c in cs {
                        if let Some(s) = c.as_str() {
                            r = r.require(s);
                        }
                    }
                }
                match self.registry.resolve(&r) {
                    Some(p) => JsonRpcResponse::ok(
                        req.id,
                        json!({
                            "bound": p.name,
                            "type": format!("{:?}", p.ptype),
                            "cost_per_call": p.cost_per_call,
                            "trust": format!("{:?}", p.trust),
                        }),
                    ),
                    None => JsonRpcResponse::err(req.id, -32000, "no provider satisfies requirement"),
                }
            }
            other => JsonRpcResponse::err(req.id, -32601, &format!("unknown method '{other}'")),
        }
    }

    /// Convenience: handle a request serialized as a JSON string, returning the
    /// response JSON string — the actual on-the-wire path.
    pub fn handle_str(&self, s: &str) -> String {
        let req: JsonRpcRequest = match serde_json::from_str(s) {
            Ok(r) => r,
            Err(e) => {
                return serde_json::to_string(&JsonRpcResponse::err(0, -32700, &format!("parse error: {e}")))
                    .unwrap()
            }
        };
        serde_json::to_string(&self.handle(&req)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Provider, ProviderType};

    fn registry() -> Registry {
        let mut r = Registry::new();
        r.register(Provider {
            name: "Tesseract".into(),
            capability: "ocr_scanner".into(),
            ptype: ProviderType::LocalWasm,
            trust: Trust::Proven,
            cost_per_call: 0.0,
            latency_ms: 120.0,
            failure_rate: 0.03,
            compliance: vec![],
        });
        r.register(Provider {
            name: "GoogleVision".into(),
            capability: "ocr_scanner".into(),
            ptype: ProviderType::RemoteApi,
            trust: Trust::Verified,
            cost_per_call: 0.015,
            latency_ms: 40.0,
            failure_rate: 0.005,
            compliance: vec![],
        });
        r
    }

    #[test]
    fn discover_over_the_wire() {
        let reg = registry();
        let server = McpServer::new(&reg);
        let req = serde_json::to_string(&JsonRpcRequest::new("discover", json!({}), 1)).unwrap();
        let resp = server.handle_str(&req);
        assert!(resp.contains("ocr_scanner"));
        assert!(resp.contains("\"id\":1"));
    }

    #[test]
    fn bind_cheapest() {
        let reg = registry();
        let server = McpServer::new(&reg);
        let resp = server.handle(&JsonRpcRequest::new(
            "bind",
            json!({"capability": "ocr_scanner", "objective": "cost"}),
            7,
        ));
        let v = resp.result.unwrap();
        assert_eq!(v["bound"], "Tesseract");
        assert_eq!(resp.id, 7);
    }

    #[test]
    fn unknown_method_errors() {
        let reg = registry();
        let server = McpServer::new(&reg);
        let resp = server.handle(&JsonRpcRequest::new("frobnicate", json!({}), 2));
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn bind_unsatisfiable_errors() {
        let reg = registry();
        let server = McpServer::new(&reg);
        let resp = server.handle(&JsonRpcRequest::new(
            "bind",
            json!({"capability": "nonexistent", "objective": "cost"}),
            3,
        ));
        assert_eq!(resp.error.unwrap().code, -32000);
    }
}
