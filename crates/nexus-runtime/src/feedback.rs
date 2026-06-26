//! The continuous knowledge loop.
//!
//! Every execution cycle feeds operational metrics — latency, cost, success —
//! back into a persistent profile keyed by node. The planner consults these
//! profiles so future workflows automatically benefit from historical behavior,
//! matching or exceeding hand-tuned systems over time. Profiles are maintained
//! as exponential moving averages, which converge toward true behavior while
//! staying responsive to drift.

use std::collections::BTreeMap;

/// A learned operational profile for one node.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub samples: u64,
    pub latency_ms: f64,
    pub cost: f64,
    pub success_rate: f64,
}

/// Smoothing factor for the EMA (0,1]; higher reacts faster to recent samples.
const ALPHA: f64 = 0.3;

/// The persistent execution knowledge graph's metric store.
#[derive(Default)]
pub struct KnowledgeLoop {
    profiles: BTreeMap<String, Profile>,
}

impl KnowledgeLoop {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one execution observation for a node.
    pub fn record(&mut self, node: &str, latency_ms: f64, cost: f64, success: bool) {
        let p = self.profiles.entry(node.to_string()).or_default();
        let s = if success { 1.0 } else { 0.0 };
        if p.samples == 0 {
            p.latency_ms = latency_ms;
            p.cost = cost;
            p.success_rate = s;
        } else {
            p.latency_ms = ema(p.latency_ms, latency_ms);
            p.cost = ema(p.cost, cost);
            p.success_rate = ema(p.success_rate, s);
        }
        p.samples += 1;
    }

    /// The learned profile for a node, if any observations exist.
    pub fn profile(&self, node: &str) -> Option<&Profile> {
        self.profiles.get(node)
    }

    /// Predicted latency for planning; falls back to `default_ms` if unseen.
    pub fn predict_latency(&self, node: &str, default_ms: f64) -> f64 {
        self.profiles.get(node).map(|p| p.latency_ms).unwrap_or(default_ms)
    }

    pub fn predict_cost(&self, node: &str, default_cost: f64) -> f64 {
        self.profiles.get(node).map(|p| p.cost).unwrap_or(default_cost)
    }
}

fn ema(prev: f64, sample: f64) -> f64 {
    ALPHA * sample + (1.0 - ALPHA) * prev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_toward_true_latency() {
        let mut k = KnowledgeLoop::new();
        // True latency is ~200ms with noise; EMA should converge near it.
        for i in 0..50 {
            let noise = if i % 2 == 0 { 10.0 } else { -10.0 };
            k.record("ChargeAccount", 200.0 + noise, 0.02, true);
        }
        let p = k.profile("ChargeAccount").unwrap();
        assert!((p.latency_ms - 200.0).abs() < 15.0, "got {}", p.latency_ms);
        assert_eq!(p.samples, 50);
    }

    #[test]
    fn unseen_node_uses_default() {
        let k = KnowledgeLoop::new();
        assert_eq!(k.predict_latency("Unknown", 123.0), 123.0);
    }

    #[test]
    fn tracks_success_rate() {
        let mut k = KnowledgeLoop::new();
        for _ in 0..20 {
            k.record("Flaky", 50.0, 0.0, true);
        }
        k.record("Flaky", 50.0, 0.0, false);
        let p = k.profile("Flaky").unwrap();
        assert!(p.success_rate < 1.0 && p.success_rate > 0.5);
    }
}
