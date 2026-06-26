//! # nexus-dsl
//!
//! The textual projection of the Nexus graph. An indentation-structured,
//! context-free DSL designed to be read and written by both humans and AI
//! models with minimal tokens. Parsing produces a [`Program`] that lowers into
//! the content-addressed [`nexus_core::GraphStore`].

pub mod block;
pub mod lexer;
pub mod parser;

pub use block::{build_blocks, Block};
pub use parser::{parse, parse_blocks, Guarantee, ParseError, Program, DECL_KEYWORDS};

/// The canonical enterprise reference program from the specification.
pub const ENTERPRISE_BILLING: &str = include_str!("../examples/enterprise_billing.nx");
