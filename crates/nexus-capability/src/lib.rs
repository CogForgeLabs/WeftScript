//! # nexus-capability
//!
//! Dynamic capability discovery and universal abstraction for Nexus: a semantic
//! [`registry`], a JSON-RPC 2.0 [`mcp`] binding layer, an AI [`router`] that
//! selects models by reasoning level and budget, and universal data routing.

pub mod mcp;
pub mod registry;
pub mod router;

pub use mcp::{JsonRpcRequest, JsonRpcResponse, McpServer};
pub use registry::{Objective, Provider, ProviderType, Registry, Requirement, Trust};
pub use router::{
    route_storage, AccessPattern, Engine, Model, ModelRouter, ReasoningLevel, RouteRequest,
};
