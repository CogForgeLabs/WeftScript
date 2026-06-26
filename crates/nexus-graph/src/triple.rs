//! Triple-graph partitioning.
//!
//! The global state is split into three decoupled graphs so that dynamic
//! runtime behavior never destabilizes static specifications:
//!
//! * **Knowledge Graph (Facts):** immutable schema — Things and Policies.
//! * **Plan Graph (Plans):** abstract solver inputs — Intents, Constraints,
//!   Capabilities, Contracts, Events.
//! * **Execution Graph (Runtime):** active processes — Workflows and Agents,
//!   plus scheduler directives.

use nexus_core::{Cid, GraphStore, Node};

/// The three partitions of a Nexus application, each a CID set into a shared
/// store.
#[derive(Debug, Default, Clone)]
pub struct TripleGraph {
    pub knowledge: Vec<Cid>,
    pub plan: Vec<Cid>,
    pub execution: Vec<Cid>,
}

/// Which partition a node kind belongs to.
pub fn partition_of(node: &Node) -> Partition {
    match node {
        Node::Thing(_) | Node::Policy(_) => Partition::Knowledge,
        Node::Intent(_)
        | Node::Constraint(_)
        | Node::Capability(_)
        | Node::Contract(_)
        | Node::Event(_) => Partition::Plan,
        Node::Workflow(_) | Node::Agent(_) | Node::Directive(_) => Partition::Execution,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Partition {
    Knowledge,
    Plan,
    Execution,
}

impl TripleGraph {
    /// Partition every node in a store into the three graphs.
    pub fn from_store(store: &GraphStore) -> TripleGraph {
        let mut t = TripleGraph::default();
        for (cid, node) in store.iter() {
            match partition_of(node) {
                Partition::Knowledge => t.knowledge.push(*cid),
                Partition::Plan => t.plan.push(*cid),
                Partition::Execution => t.execution.push(*cid),
            }
        }
        t
    }

    pub fn total(&self) -> usize {
        self.knowledge.len() + self.plan.len() + self.execution.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_dsl::{parse, ENTERPRISE_BILLING};

    #[test]
    fn partitions_reference_program() {
        let store = parse(ENTERPRISE_BILLING).unwrap().to_store();
        let t = TripleGraph::from_store(&store);
        // 3 things + 2 policies = 5 facts.
        assert_eq!(t.knowledge.len(), 5);
        // 1 intent + 2 constraints + 1 capability + 1 contract + 3 events = 8.
        assert_eq!(t.plan.len(), 8);
        // 1 workflow + 2 agents + 4 directives = 7.
        assert_eq!(t.execution.len(), 7);
        assert_eq!(t.total(), store.len());
    }
}
