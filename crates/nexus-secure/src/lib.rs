//! # nexus-secure
//!
//! Built-in SECURITY, PRIVACY, and ANONYMITY primitives for Nexus, with zero
//! external dependencies beyond `nexus-core` and `blake3`.
//!
//! * [`redact`] — PII / secret detection and redaction (PRIVACY).
//! * [`Sandbox`] — capability sandbox for commands and hosts (SECURITY).
//! * [`anonymize`] / [`anon_token`] — pseudonyms and ephemeral tokens
//!   (ANONYMITY).
//!
//! All detectors are hand-written (no regex crate is available in the
//! workspace).

pub mod anon;
pub mod redact;
pub mod sandbox;

pub use anon::{anon_token, anonymize, anonymous_headers, proxy_from_env};
pub use redact::{contains_pii, pii_kinds, redact};
pub use sandbox::Sandbox;
