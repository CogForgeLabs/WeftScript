//! # nexus-verify
//!
//! SMT-style formal verification for the Nexus compiler. Provides an exact
//! rational linear-arithmetic feasibility [`solver`] (Fourier–Motzkin with
//! model extraction) and a [`symbolic`] layer that proves safety guarantees by
//! refuting their negation, producing concrete counterexamples on failure.

pub mod solver;
pub mod symbolic;

pub use solver::{solve, Atom, Feasibility, LinExpr, Rel};
pub use symbolic::{format_model, prove, ProofResult};
