//! # nexus-dashboard
//!
//! An **optional**, auto-generated web dashboard for Nexus. It is headless and
//! inert by default: nothing binds a socket or serves a page until a caller
//! explicitly invokes [`serve`]/[`Server::run`] (the CLI gates this behind a
//! flag wired up elsewhere). Pulling this crate into a build adds no behavior on
//! its own.
//!
//! Three layers, each independently usable and zero external dependencies
//! (only `std`, `nexus-core`, `nexus-app`):
//!
//! * [`route`] — a pure `(method, path, body) -> HttpReply` router that *is* the
//!   dashboard: it serves a self-contained HTML/CSS/JS page at `/`, and JSON at
//!   `/api/output`, `/api/trace`, and `/api/run`. Fully unit-testable, no I/O.
//! * [`set_output`]/[`push_output`]/[`output`]/[`clear_output`] — a process-global
//!   capture buffer so a headless run's printed lines can be surfaced.
//! * [`Server`]/[`bind`]/[`serve`] — a minimal HTTP/1.0 server over
//!   `std::net::TcpListener` that drives [`route`].

mod output;
mod router;
mod server;

pub use output::{clear_output, output, push_output, set_output};
pub use router::{json_escape, route, HttpReply};
pub use server::{bind, serve, Server};
