//! # nexus-core
//!
//! The content-addressed graph core of the Nexus Universal Intent Operating
//! System. Provides:
//!
//! * [`Cid`] — 512-bit BLAKE3 content identifiers.
//! * The seven true primitives ([`Thing`], [`Intent`], [`Constraint`],
//!   [`Policy`], [`Event`], [`Capability`], [`Contract`]) plus composite
//!   [`Workflow`] and [`Agent`] constructs.
//! * [`Node`] — the unified, content-addressed node enum.
//! * [`GraphStore`] — an append-only store with automatic structural
//!   deduplication and a name index.
//!
//! Everything compiles into the *same* directed graph structure, regardless of
//! whether it was authored as text, drawn as a diagram, or emitted by an AI
//! agent.

pub mod canonical;
pub mod cid;
pub mod node;
pub mod primitives;
pub mod store;
pub mod trace;
pub mod value;

pub use canonical::{Canonical, CanonicalWriter};
pub use cid::Cid;
pub use node::Node;
pub use primitives::*;
pub use store::{Edge, GraphStore};
pub use value::{BinOp, CmpOp, Expr, Literal, Predicate, Rational, Ty};
