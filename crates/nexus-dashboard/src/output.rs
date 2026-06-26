//! Process-global, thread-safe capture buffer for a headless program's output.
//!
//! A program run with the dashboard disabled still produces printed lines; this
//! buffer is where those lines are stashed so the (optional) dashboard can show
//! them. It is a single global guarded by a mutex — cheap, lock-only-on-touch,
//! and entirely independent of whether a server is running.

use std::sync::{Mutex, OnceLock};

fn buffer() -> &'static Mutex<Vec<String>> {
    static BUF: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    BUF.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the entire captured output with `lines`.
pub fn set_output(lines: Vec<String>) {
    let mut b = buffer().lock().unwrap();
    *b = lines;
}

/// Append a single line to the captured output.
pub fn push_output(line: impl Into<String>) {
    buffer().lock().unwrap().push(line.into());
}

/// Snapshot the captured output.
pub fn output() -> Vec<String> {
    buffer().lock().unwrap().clone()
}

/// Drop all captured output.
pub fn clear_output() {
    buffer().lock().unwrap().clear();
}
