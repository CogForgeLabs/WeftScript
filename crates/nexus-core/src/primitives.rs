//! The seven true primitives plus the composite Workflow and Agent constructs.
//!
//! Any Nexus application is a composition of these typed objects. Each is
//! content-addressed: its CID is derived from its canonical encoding.

use crate::canonical::{Canonical, CanonicalWriter};
use crate::value::{Literal, Predicate, Ty};
use serde::{Deserialize, Serialize};

/// A structural field of a `thing`, optionally governed by a named policy.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
    pub policy: Option<String>,
}

impl Canonical for Field {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        self.ty.write_canonical(w);
        match &self.policy {
            Some(p) => {
                w.tag(1).str(p);
            }
            None => {
                w.tag(0);
            }
        }
    }
}

/// `thing` — structural schemas, state properties, and interfaces.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Thing {
    pub name: String,
    pub implements: Option<String>,
    pub fields: Vec<Field>,
}

impl Canonical for Thing {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        match &self.implements {
            Some(i) => {
                w.tag(1).str(i);
            }
            None => {
                w.tag(0);
            }
        }
        w.seq(self.fields.len());
        for f in &self.fields {
            f.write_canonical(w);
        }
    }
}

/// `intent` — a functional goal or state transition to execute.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    pub inputs: Vec<(String, Ty)>,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
}

impl Canonical for Intent {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.inputs.len());
        for (n, t) in &self.inputs {
            w.str(n);
            t.write_canonical(w);
        }
        w.seq(self.goals.len());
        for g in &self.goals {
            w.str(g);
        }
        w.seq(self.constraints.len());
        for c in &self.constraints {
            w.str(c);
        }
    }
}

/// `constraint` — a boundary condition expressed as comparison predicates.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    pub predicates: Vec<Predicate>,
}

impl Canonical for Constraint {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.predicates.len());
        for p in &self.predicates {
            p.write_canonical(w);
        }
    }
}

/// A single directive inside a security `policy`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum SecurityDirective {
    /// `restrict <action> -> <target>`
    Restrict { action: String, target: String },
    /// `encrypt <Thing.field>`
    Encrypt(String),
    /// `audit <bool>`
    Audit(bool),
    /// `trust <level>`
    Trust(String),
    /// `default_deny <bool>`
    DefaultDeny(bool),
}

impl Canonical for SecurityDirective {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        match self {
            SecurityDirective::Restrict { action, target } => {
                w.tag(0).str(action).str(target);
            }
            SecurityDirective::Encrypt(t) => {
                w.tag(1).str(t);
            }
            SecurityDirective::Audit(b) => {
                w.tag(2).u64(*b as u64);
            }
            SecurityDirective::Trust(t) => {
                w.tag(3).str(t);
            }
            SecurityDirective::DefaultDeny(b) => {
                w.tag(4).u64(*b as u64);
            }
        }
    }
}

/// `policy` — security, access rights, and governance boundaries.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub directives: Vec<SecurityDirective>,
}

impl Canonical for Policy {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.directives.len());
        for d in &self.directives {
            d.write_canonical(w);
        }
    }
}

/// `event` — an asynchronous signal name.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
}

impl Canonical for Event {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
    }
}

/// The input/output signature of a capability.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Signature {
    pub inputs: Vec<(String, Ty)>,
    pub outputs: Vec<(String, Ty)>,
}

impl Canonical for Signature {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.seq(self.inputs.len());
        for (n, t) in &self.inputs {
            w.str(n);
            t.write_canonical(w);
        }
        w.seq(self.outputs.len());
        for (n, t) in &self.outputs {
            w.str(n);
            t.write_canonical(w);
        }
    }
}

/// `capability` — an abstract functional requirement for runtime matching.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub signature: Signature,
    pub guarantees: Vec<(String, Literal)>,
}

impl Canonical for Capability {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        self.signature.write_canonical(w);
        w.seq(self.guarantees.len());
        for (k, v) in &self.guarantees {
            w.str(k);
            v.write_canonical(w);
        }
    }
}

/// `contract` — performance, cost, and safety resource boundaries.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub name: String,
    pub limits: Vec<(String, Literal)>,
}

impl Canonical for Contract {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.limits.len());
        for (k, v) in &self.limits {
            w.str(k);
            v.write_canonical(w);
        }
    }
}

/// Fallback behavior attached to a workflow step.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Fallback {
    pub retry: Option<u32>,
    pub backoff: Option<String>,
    pub on_failure: Option<String>,
}

impl Canonical for Fallback {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        match self.retry {
            Some(r) => {
                w.tag(1).u64(r as u64);
            }
            None => {
                w.tag(0);
            }
        }
        opt_str(w, &self.backoff);
        opt_str(w, &self.on_failure);
    }
}

/// A step within a re-entrant workflow.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Step {
    pub name: String,
    pub intent: Option<String>,
    pub agent: Option<String>,
    pub need: Option<String>,
    pub contract: Option<String>,
    pub deterministic: Option<bool>,
    pub reasoning: Option<String>,
    pub tools: Vec<String>,
    pub fallback: Option<Fallback>,
}

impl Canonical for Step {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        opt_str(w, &self.intent);
        opt_str(w, &self.agent);
        opt_str(w, &self.need);
        opt_str(w, &self.contract);
        match self.deterministic {
            Some(b) => {
                w.tag(1).u64(b as u64);
            }
            None => {
                w.tag(0);
            }
        }
        opt_str(w, &self.reasoning);
        w.seq(self.tools.len());
        for t in &self.tools {
            w.str(t);
        }
        match &self.fallback {
            Some(fb) => {
                w.tag(1);
                fb.write_canonical(w);
            }
            None => {
                w.tag(0);
            }
        }
    }
}

/// `workflow` — a re-entrant DCR-style process triggered by an event.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub trigger: String,
    pub steps: Vec<Step>,
}

impl Canonical for Workflow {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.str(&self.trigger);
        w.seq(self.steps.len());
        for s in &self.steps {
            s.write_canonical(w);
        }
    }
}

/// Whether a metric objective should be maximized or minimized.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Objective {
    Maximize,
    Minimize,
}

/// A named optimization metric with maximize/minimize objectives.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub objectives: Vec<(Objective, String)>,
}

impl Canonical for Metric {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.objectives.len());
        for (o, t) in &self.objectives {
            w.tag(*o as u8).str(t);
        }
    }
}

/// `agent` — a goal-directed probabilistic collaborator with metrics & tools.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub goals: Vec<String>,
    pub metric: Option<Metric>,
    pub tools: Vec<String>,
    pub constraints: Vec<String>,
}

impl Canonical for Agent {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        w.str(&self.name);
        w.seq(self.goals.len());
        for g in &self.goals {
            w.str(g);
        }
        match &self.metric {
            Some(m) => {
                w.tag(1);
                m.write_canonical(w);
            }
            None => {
                w.tag(0);
            }
        }
        w.seq(self.tools.len());
        for t in &self.tools {
            w.str(t);
        }
        w.seq(self.constraints.len());
        for c in &self.constraints {
            w.str(c);
        }
    }
}

/// A top-level scheduler / observability / optimization directive.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Directive {
    /// `schedule <workflow> <mode>`
    Schedule { target: String, mode: String },
    /// `observe <mode>`
    Observe(String),
    /// `optimize <objective>`
    Optimize(String),
    /// `secure <mode>`
    Secure(String),
}

impl Canonical for Directive {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        match self {
            Directive::Schedule { target, mode } => {
                w.tag(0).str(target).str(mode);
            }
            Directive::Observe(m) => {
                w.tag(1).str(m);
            }
            Directive::Optimize(m) => {
                w.tag(2).str(m);
            }
            Directive::Secure(m) => {
                w.tag(3).str(m);
            }
        }
    }
}

fn opt_str(w: &mut CanonicalWriter, o: &Option<String>) {
    match o {
        Some(s) => {
            w.tag(1).str(s);
        }
        None => {
            w.tag(0);
        }
    }
}
