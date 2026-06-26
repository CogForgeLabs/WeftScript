//! # nexus-runtime
//!
//! The local-first execution engine for Nexus: dependency-driven
//! auto-parallel execution, cryptographic provenance, resilient retry/backoff,
//! and the continuous knowledge loop that feeds metrics back into planning.

pub mod engine;
pub mod fallback;
pub mod feedback;
pub mod provenance;

pub use engine::{
    determinism_split, execute_parallel, execute_serial, ExecReport, FnOp, NodeOp,
};
pub use fallback::{run_with_retry, FailureAction, RetryOutcome};
pub use feedback::{KnowledgeLoop, Profile};
pub use provenance::{trace, Lineage};
