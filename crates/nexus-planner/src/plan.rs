//! Lowering a workflow into a schedulable task DAG.

use crate::dag::TaskDag;
use nexus_core::Workflow;

/// Build a task DAG from a workflow. Steps form a dependency chain (each step
/// consumes the prior step's output); probabilistic agent steps are weighted
/// more heavily than deterministic ones to reflect their real latency/cost.
pub fn from_workflow(wf: &Workflow) -> TaskDag {
    let mut g = TaskDag::new();
    let mut prev = None;
    for step in &wf.steps {
        let work = if step.agent.is_some() {
            5.0 // probabilistic / LLM-backed step
        } else if step.deterministic == Some(true) {
            1.0 // verified deterministic compute
        } else {
            2.0 // capability-backed remote call
        };
        let id = g.add_task(&step.name, work);
        if let Some(p) = prev {
            g.add_dep(p, id, 1.0);
        }
        prev = Some(id);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::Node;
    use nexus_dsl::{parse, ENTERPRISE_BILLING};

    #[test]
    fn builds_dag_from_reference_workflow() {
        let prog = parse(ENTERPRISE_BILLING).unwrap();
        let wf = prog.nodes.iter().find_map(|n| match n {
            Node::Workflow(w) => Some(w),
            _ => None,
        }).unwrap();
        let g = from_workflow(wf);
        assert_eq!(g.len(), 3);
        assert!(g.is_dag());
        // Linear chain -> width 1, critical path == serial work.
        assert_eq!(g.max_width(), 1);
        assert_eq!(g.critical_path(), g.serial_work());
    }
}
