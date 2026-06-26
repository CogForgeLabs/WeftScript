//! # nexus-exec
//!
//! The executable layer that turns Nexus from a declarative/verification
//! substrate into a general-purpose, runnable language. An `app` block holds
//! `fn` definitions and a `main` entry; the interpreter evaluates it with a
//! broad standard library (math, strings, lists, maps, control flow) and
//! captures printed output.
//!
//! ```
//! let src = "app Demo\n    main\n        print 2 + 3 * 4\n";
//! let app = nexus_exec::parse_app(src).unwrap();
//! let out = nexus_exec::run(&app).unwrap();
//! assert_eq!(out, vec!["14"]);
//! ```

pub mod ast;
pub mod builtins;
pub mod interp;
pub mod json;
pub mod parser;
pub mod value;

pub use ast::App;
pub use interp::{GuardedOutcome, HostFns, Interp, Limits};
pub use parser::{app_from_blocks, parse_app, ExecParseError};
pub use value::Value;

/// Parse and run a program under hard resource limits with rollback/recovery.
pub fn run_guarded(src: &str, limits: Limits) -> Result<GuardedOutcome, String> {
    let app = parse_app(src).map_err(|e| e.to_string())?;
    Ok(Interp::run_guarded(&app, limits))
}

/// Parse and run a program from source, returning its printed output lines.
pub fn run_source(src: &str) -> Result<Vec<String>, String> {
    let app = parse_app(src).map_err(|e| e.to_string())?;
    Interp::run(&app)
}

/// Run an already-parsed app.
pub fn run(app: &App) -> Result<Vec<String>, String> {
    Interp::run(app)
}
