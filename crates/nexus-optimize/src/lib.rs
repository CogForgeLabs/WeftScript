//! # nexus-optimize
//!
//! The model-driven optimization surface of Nexus: Triple Graph Grammar view
//! synchronization ([`tgg`]), a DSL [`printer`], TuRBO Bayesian prompt
//! optimization ([`turbo`]), and digital-twin [`simulate`]ion.

pub mod printer;
pub mod simulate;
pub mod tgg;
pub mod turbo;

pub use printer::to_dsl;
pub use simulate::{simulate, ImpactReport};
pub use tgg::TripleView;
pub use turbo::{maximize, Bounds, OptResult};
