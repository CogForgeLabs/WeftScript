//! # nexus-graph
//!
//! Triple-graph partitioning, Semantic Git, and time-travel for the Nexus
//! content-addressed graph.

pub mod history;
pub mod semantic;
pub mod triple;

pub use history::{Commit, VersionedGraph};
pub use semantic::{diff, merge, MergeConflict, MergeResult, SemanticPatch};
pub use triple::{partition_of, Partition, TripleGraph};
