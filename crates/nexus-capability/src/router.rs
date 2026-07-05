//! The AI Router and universal data routing.
//!
//! Rather than hardcoding `gpt-4o` or `import postgres`, developers declare a
//! logical reasoning requirement or a `store` directive. The runtime selects
//! the optimal model (by cost bounds, token budget, and availability) and the
//! optimal storage engine (by schema and access pattern).

use serde::{Deserialize, Serialize};

/// A generative model the router can select.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub cost_per_1k_in: f64,
    pub cost_per_1k_out: f64,
    /// Relative quality/capability score in [0,1].
    pub quality: f64,
    pub context_window: u32,
    pub latency_ms: f64,
    pub available: bool,
}

/// Logical reasoning levels declared in the DSL (`reasoning level.deep_reasoning`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReasoningLevel {
    DeepReasoning,
    Balanced,
    CostOptimized,
    Fast,
}

impl ReasoningLevel {
    pub fn parse(s: &str) -> Option<ReasoningLevel> {
        Some(match s.trim_start_matches("level.") {
            "deep_reasoning" | "deep" => ReasoningLevel::DeepReasoning,
            "balanced" => ReasoningLevel::Balanced,
            "cost_optimized" | "cost" => ReasoningLevel::CostOptimized,
            "fast" => ReasoningLevel::Fast,
            _ => return None,
        })
    }
}

/// A routing request describing the task's token shape and bounds.
#[derive(Clone, Debug)]
pub struct RouteRequest {
    pub level: ReasoningLevel,
    pub est_input_tokens: u32,
    pub est_output_tokens: u32,
    pub max_cost: Option<f64>,
}

impl RouteRequest {
    pub fn cost_on(&self, m: &Model) -> f64 {
        (self.est_input_tokens as f64 / 1000.0) * m.cost_per_1k_in
            + (self.est_output_tokens as f64 / 1000.0) * m.cost_per_1k_out
    }
}

#[derive(Default)]
pub struct ModelRouter {
    models: Vec<Model>,
}

impl ModelRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, m: Model) {
        self.models.push(m);
    }

    /// Select the optimal model for a request, honoring availability, the
    /// context window, and the cost ceiling, then optimizing by reasoning level.
    pub fn route(&self, req: &RouteRequest) -> Option<&Model> {
        let needed = req.est_input_tokens + req.est_output_tokens;
        let feasible: Vec<&Model> = self
            .models
            .iter()
            .filter(|m| m.available)
            .filter(|m| m.context_window >= needed)
            .filter(|m| req.max_cost.is_none_or(|c| req.cost_on(m) <= c))
            .collect();
        feasible.into_iter().min_by(|a, b| {
            let ka = self.key(req, a);
            let kb = self.key(req, b);
            ka.partial_cmp(&kb).unwrap().then(a.name.cmp(&b.name))
        })
    }

    /// Minimization key per reasoning level.
    fn key(&self, req: &RouteRequest, m: &Model) -> f64 {
        match req.level {
            // Highest quality wins (minimize negative quality).
            ReasoningLevel::DeepReasoning => -m.quality,
            // Best quality-per-dollar (minimize cost/quality).
            ReasoningLevel::Balanced => req.cost_on(m) / m.quality.max(1e-6),
            // Cheapest wins.
            ReasoningLevel::CostOptimized => req.cost_on(m),
            // Lowest latency wins.
            ReasoningLevel::Fast => m.latency_ms,
        }
    }
}

/// How data is accessed, driving the storage-engine choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessPattern {
    StructuredQuery,
    Analytical,
    KeyValue,
    SemanticSearch,
}

/// A storage engine the runtime can route a `store` declaration to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    Postgres,
    DuckDB,
    Sqlite,
    Redis,
    RocksDB,
    Vector,
}

/// Route a `store` declaration to the most efficient engine given the access
/// pattern, data volume, and the local-first preference.
pub fn route_storage(pattern: AccessPattern, large_volume: bool, local_first: bool) -> Engine {
    match pattern {
        AccessPattern::StructuredQuery => {
            if local_first {
                Engine::Sqlite
            } else {
                Engine::Postgres
            }
        }
        AccessPattern::Analytical => Engine::DuckDB,
        AccessPattern::KeyValue => {
            if local_first || !large_volume {
                Engine::RocksDB
            } else {
                Engine::Redis
            }
        }
        AccessPattern::SemanticSearch => Engine::Vector,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router() -> ModelRouter {
        let mut r = ModelRouter::new();
        r.register(Model {
            name: "opus".into(),
            cost_per_1k_in: 0.015,
            cost_per_1k_out: 0.075,
            quality: 0.98,
            context_window: 200_000,
            latency_ms: 900.0,
            available: true,
        });
        r.register(Model {
            name: "haiku".into(),
            cost_per_1k_in: 0.0008,
            cost_per_1k_out: 0.004,
            quality: 0.82,
            context_window: 200_000,
            latency_ms: 250.0,
            available: true,
        });
        r
    }

    #[test]
    fn deep_reasoning_picks_quality() {
        let r = router();
        let req = RouteRequest { level: ReasoningLevel::DeepReasoning, est_input_tokens: 1000, est_output_tokens: 500, max_cost: None };
        assert_eq!(r.route(&req).unwrap().name, "opus");
    }

    #[test]
    fn cost_optimized_picks_cheap() {
        let r = router();
        let req = RouteRequest { level: ReasoningLevel::CostOptimized, est_input_tokens: 1000, est_output_tokens: 500, max_cost: None };
        assert_eq!(r.route(&req).unwrap().name, "haiku");
    }

    #[test]
    fn cost_ceiling_excludes_expensive() {
        let r = router();
        // Budget too small for opus on this token shape -> falls back to haiku.
        let req = RouteRequest { level: ReasoningLevel::DeepReasoning, est_input_tokens: 1000, est_output_tokens: 1000, max_cost: Some(0.01) };
        assert_eq!(r.route(&req).unwrap().name, "haiku");
    }

    #[test]
    fn fast_picks_low_latency() {
        let r = router();
        let req = RouteRequest { level: ReasoningLevel::Fast, est_input_tokens: 10, est_output_tokens: 10, max_cost: None };
        assert_eq!(r.route(&req).unwrap().name, "haiku");
    }

    #[test]
    fn storage_routing() {
        assert_eq!(route_storage(AccessPattern::StructuredQuery, false, true), Engine::Sqlite);
        assert_eq!(route_storage(AccessPattern::StructuredQuery, true, false), Engine::Postgres);
        assert_eq!(route_storage(AccessPattern::Analytical, true, false), Engine::DuckDB);
        assert_eq!(route_storage(AccessPattern::SemanticSearch, false, false), Engine::Vector);
        assert_eq!(route_storage(AccessPattern::KeyValue, true, false), Engine::Redis);
    }
}
