//! The unified node type: every primitive projected into one content-addressed
//! enum so the whole application is a single typed graph.

use crate::canonical::{Canonical, CanonicalWriter};
use crate::cid::Cid;
use crate::primitives::*;
use serde::{Deserialize, Serialize};

/// A single content-addressed node in the unified graph.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Node {
    Thing(Thing),
    Intent(Intent),
    Constraint(Constraint),
    Policy(Policy),
    Event(Event),
    Capability(Capability),
    Contract(Contract),
    Workflow(Workflow),
    Agent(Agent),
    Directive(Directive),
}

impl Canonical for Node {
    fn write_canonical(&self, w: &mut CanonicalWriter) {
        // A leading kind tag guarantees two differently-typed nodes can never
        // collide even if their inner encodings coincide.
        match self {
            Node::Thing(x) => {
                w.tag(0xA0);
                x.write_canonical(w);
            }
            Node::Intent(x) => {
                w.tag(0xA1);
                x.write_canonical(w);
            }
            Node::Constraint(x) => {
                w.tag(0xA2);
                x.write_canonical(w);
            }
            Node::Policy(x) => {
                w.tag(0xA3);
                x.write_canonical(w);
            }
            Node::Event(x) => {
                w.tag(0xA4);
                x.write_canonical(w);
            }
            Node::Capability(x) => {
                w.tag(0xA5);
                x.write_canonical(w);
            }
            Node::Contract(x) => {
                w.tag(0xA6);
                x.write_canonical(w);
            }
            Node::Workflow(x) => {
                w.tag(0xA7);
                x.write_canonical(w);
            }
            Node::Agent(x) => {
                w.tag(0xA8);
                x.write_canonical(w);
            }
            Node::Directive(x) => {
                w.tag(0xA9);
                x.write_canonical(w);
            }
        }
    }
}

impl Node {
    /// The content identifier of this node, derived purely from its content.
    pub fn cid(&self) -> Cid {
        Cid::of_bytes(&self.canonical_bytes())
    }

    /// The declared name of the node, if it has one (Directives are anonymous).
    pub fn name(&self) -> Option<&str> {
        Some(match self {
            Node::Thing(x) => &x.name,
            Node::Intent(x) => &x.name,
            Node::Constraint(x) => &x.name,
            Node::Policy(x) => &x.name,
            Node::Event(x) => &x.name,
            Node::Capability(x) => &x.name,
            Node::Contract(x) => &x.name,
            Node::Workflow(x) => &x.name,
            Node::Agent(x) => &x.name,
            Node::Directive(_) => return None,
        })
    }

    /// A stable, human/AI-readable label for the node's kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Node::Thing(_) => "thing",
            Node::Intent(_) => "intent",
            Node::Constraint(_) => "constraint",
            Node::Policy(_) => "policy",
            Node::Event(_) => "event",
            Node::Capability(_) => "capability",
            Node::Contract(_) => "contract",
            Node::Workflow(_) => "workflow",
            Node::Agent(_) => "agent",
            Node::Directive(_) => "directive",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Ty;

    #[test]
    fn identical_things_share_cid() {
        let a = Node::Thing(Thing {
            name: "Account".into(),
            implements: None,
            fields: vec![Field { name: "balance".into(), ty: Ty::Decimal, policy: None }],
        });
        let b = a.clone();
        assert_eq!(a.cid(), b.cid());
    }

    #[test]
    fn different_kind_same_name_differ() {
        let t = Node::Event(Event { name: "X".into() });
        let i = Node::Intent(Intent {
            name: "X".into(),
            inputs: vec![],
            goals: vec![],
            constraints: vec![],
        });
        assert_ne!(t.cid(), i.cid());
    }
}
