//! Semantic capability discovery.
//!
//! Instead of static library imports, components declare an abstract functional
//! *need*. The registry holds providers that satisfy capabilities, each tagged
//! with type, trust, cost, latency, and compliance, and binds the most
//! efficient provider for the active execution context — exactly the
//! `need: ocr_scanner` → {Tesseract, Google Vision, AWS Textract} resolution
//! from the specification.

use serde::{Deserialize, Serialize};

/// How a provider executes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ProviderType {
    /// Runs locally (e.g. a WASM module) — zero network, local-first.
    LocalWasm,
    /// A remote API call.
    RemoteApi,
}

/// Trust tiers, ordered from most to least trusted.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Trust {
    Unverified = 0,
    Reviewed = 1,
    Verified = 2,
    Proven = 3,
}

/// A concrete provider that satisfies a capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub capability: String,
    pub ptype: ProviderType,
    pub trust: Trust,
    pub cost_per_call: f64,
    pub latency_ms: f64,
    pub failure_rate: f64,
    pub compliance: Vec<String>,
}

/// What the optimizer should minimize/maximize when binding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Objective {
    Cost,
    Latency,
    Trust,
    Reliability,
}

/// A declared functional need plus binding policy.
#[derive(Clone, Debug)]
pub struct Requirement {
    pub capability: String,
    pub min_trust: Trust,
    pub required_compliance: Vec<String>,
    pub max_cost: Option<f64>,
    pub max_failure_rate: Option<f64>,
    pub objective: Objective,
}

impl Requirement {
    pub fn new(capability: &str, objective: Objective) -> Self {
        Requirement {
            capability: capability.into(),
            min_trust: Trust::Reviewed,
            required_compliance: Vec::new(),
            max_cost: None,
            max_failure_rate: None,
            objective,
        }
    }
    pub fn min_trust(mut self, t: Trust) -> Self {
        self.min_trust = t;
        self
    }
    pub fn require(mut self, c: &str) -> Self {
        self.required_compliance.push(c.into());
        self
    }
    pub fn max_failure_rate(mut self, r: f64) -> Self {
        self.max_failure_rate = Some(r);
        self
    }
}

#[derive(Default)]
pub struct Registry {
    providers: Vec<Provider>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, p: Provider) {
        self.providers.push(p);
    }

    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    /// All distinct capability names known to the registry.
    pub fn capabilities(&self) -> Vec<String> {
        let mut v: Vec<String> = self.providers.iter().map(|p| p.capability.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Providers eligible to satisfy a requirement (before optimization).
    pub fn candidates(&self, req: &Requirement) -> Vec<&Provider> {
        self.providers
            .iter()
            .filter(|p| p.capability == req.capability)
            .filter(|p| p.trust >= req.min_trust)
            .filter(|p| req.required_compliance.iter().all(|c| p.compliance.contains(c)))
            .filter(|p| req.max_cost.map_or(true, |m| p.cost_per_call <= m))
            .filter(|p| req.max_failure_rate.map_or(true, |m| p.failure_rate <= m))
            .collect()
    }

    /// Bind the single best provider for a requirement under its objective.
    pub fn resolve(&self, req: &Requirement) -> Option<&Provider> {
        let cands = self.candidates(req);
        cands.into_iter().min_by(|a, b| {
            let ka = score(a, req.objective);
            let kb = score(b, req.objective);
            ka.partial_cmp(&kb).unwrap().then(a.name.cmp(&b.name))
        })
    }
}

/// Lower score = better; objectives are turned into a minimization key.
fn score(p: &Provider, obj: Objective) -> f64 {
    match obj {
        Objective::Cost => p.cost_per_call,
        Objective::Latency => p.latency_ms,
        Objective::Trust => -(p.trust as i32 as f64),
        Objective::Reliability => p.failure_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ocr_registry() -> Registry {
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
            compliance: vec!["SOC2".into()],
        });
        r.register(Provider {
            name: "AWSTextract".into(),
            capability: "ocr_scanner".into(),
            ptype: ProviderType::RemoteApi,
            trust: Trust::Reviewed,
            cost_per_call: 0.025,
            latency_ms: 60.0,
            failure_rate: 0.004,
            compliance: vec!["SOC2".into(), "HIPAA".into()],
        });
        r
    }

    #[test]
    fn cheapest_is_local() {
        let r = ocr_registry();
        let req = Requirement::new("ocr_scanner", Objective::Cost);
        assert_eq!(r.resolve(&req).unwrap().name, "Tesseract");
    }

    #[test]
    fn lowest_latency_is_google() {
        let r = ocr_registry();
        let req = Requirement::new("ocr_scanner", Objective::Latency);
        assert_eq!(r.resolve(&req).unwrap().name, "GoogleVision");
    }

    #[test]
    fn compliance_filter_forces_textract() {
        let r = ocr_registry();
        let req = Requirement::new("ocr_scanner", Objective::Cost).require("HIPAA");
        assert_eq!(r.resolve(&req).unwrap().name, "AWSTextract");
    }

    #[test]
    fn no_provider_when_overconstrained() {
        let r = ocr_registry();
        let req = Requirement::new("ocr_scanner", Objective::Cost)
            .min_trust(Trust::Proven)
            .require("HIPAA");
        assert!(r.resolve(&req).is_none());
    }
}
