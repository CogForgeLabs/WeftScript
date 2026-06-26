//! Triple-View synchronization via a Triple Graph Grammar.
//!
//! The platform maintains three bi-directionally synchronized projections of one
//! underlying graph: the **Graph View** (source), the **DSL View** (text), and
//! the **Generated Runtime View**. An edit in any view is propagated to the
//! others. The correspondence is structural identity over content-addressed
//! CIDs, so consistency is *checkable*, not merely asserted.

use crate::printer::to_dsl;
use nexus_core::GraphStore;
use nexus_dsl::{parse, ParseError, Program};

/// A synchronized triple view over one program.
pub struct TripleView {
    program: Program,
}

impl TripleView {
    /// Build from DSL text (edit originating in the DSL view).
    pub fn from_dsl(src: &str) -> Result<TripleView, ParseError> {
        Ok(TripleView { program: parse(src)? })
    }

    /// Build from a program (edit originating in the graph view).
    pub fn from_program(program: Program) -> TripleView {
        TripleView { program }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The Graph View (source S): the content-addressed store.
    pub fn graph(&self) -> GraphStore {
        self.program.to_store()
    }

    /// The DSL View (target T): canonical text projection.
    pub fn dsl(&self) -> String {
        to_dsl(&self.program)
    }

    /// The Generated Runtime View: live scheduler/observability configuration
    /// derived from execution-graph directives.
    pub fn runtime_view(&self) -> String {
        let mut s = String::from("runtime:\n");
        for n in &self.program.nodes {
            if let nexus_core::Node::Workflow(w) = n {
                s.push_str(&format!("  workflow {} ({} steps)\n", w.name, w.steps.len()));
            }
        }
        for d in &self.program.directives {
            s.push_str(&format!("  {:?}\n", d));
        }
        s
    }

    /// Propagate a textual edit: reparse the DSL and update the graph/runtime.
    pub fn edit_dsl(&mut self, new_src: &str) -> Result<(), ParseError> {
        self.program = parse(new_src)?;
        Ok(())
    }

    /// Propagate a graph edit: replace the program; DSL/runtime re-project on
    /// the next read.
    pub fn edit_graph(&mut self, program: Program) {
        self.program = program;
    }

    /// The TGG consistency invariant: the DSL projection re-parses to a graph
    /// with the exact same multiset of node CIDs (round-trip stability).
    pub fn consistent(&self) -> bool {
        match parse(&self.dsl()) {
            Ok(reparsed) => {
                let mut a: Vec<String> =
                    self.program.to_store().iter().map(|(c, _)| c.to_hex()).collect();
                let mut b: Vec<String> =
                    reparsed.to_store().iter().map(|(c, _)| c.to_hex()).collect();
                a.sort();
                b.sort();
                a == b
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_dsl::ENTERPRISE_BILLING;

    #[test]
    fn reference_program_round_trips() {
        let view = TripleView::from_dsl(ENTERPRISE_BILLING).unwrap();
        // Graph -> DSL -> Graph reproduces every node CID.
        assert!(view.consistent(), "TGG round-trip must preserve all CIDs");
    }

    #[test]
    fn dsl_edit_propagates_to_graph() {
        let mut view = TripleView::from_dsl("project P\nevent A\n").unwrap();
        let before = view.graph().len();
        view.edit_dsl("project P\nevent A\nevent B\n").unwrap();
        assert_eq!(view.graph().len(), before + 1);
        assert!(view.consistent());
    }

    #[test]
    fn runtime_view_reflects_workflows() {
        let view = TripleView::from_dsl(ENTERPRISE_BILLING).unwrap();
        let rv = view.runtime_view();
        assert!(rv.contains("StandardInvoiceSettlement"));
        assert!(rv.contains("Schedule"));
    }

    #[test]
    fn printed_dsl_is_reparseable() {
        let view = TripleView::from_dsl(ENTERPRISE_BILLING).unwrap();
        let text = view.dsl();
        // The projection must itself be valid Nexus DSL.
        assert!(parse(&text).is_ok());
    }
}
