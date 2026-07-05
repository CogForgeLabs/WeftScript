//! Zero-dependency structured tracing for the whole system.
//!
//! Everything — the interpreter, the hardware dispatcher, the planner, the tool
//! runner, the resource guard — emits structured records through this one sink
//! so any run can be replayed and traced end to end. Records are leveled,
//! ordered by a monotonic sequence number, timestamped by elapsed microseconds
//! since process start, and carry an optional span id plus arbitrary key/value
//! fields.
//!
//! The sink is global and thread-safe (an `RwLock`-guarded ring buffer), so the
//! parallel builtins and worker threads all feed the same trace. Emission to
//! stderr is opt-in; capture into the in-memory buffer is always on (bounded).

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

/// Severity levels, ordered so a threshold filters everything below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }

    /// Parse a level name (case-insensitive); unknown names fall back to `Info`.
    pub fn parse(s: &str) -> Level {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Level::Trace,
            "debug" => Level::Debug,
            "warn" | "warning" => Level::Warn,
            "error" | "err" => Level::Error,
            _ => Level::Info,
        }
    }
}

/// A single captured trace record.
#[derive(Clone, Debug)]
pub struct Record {
    pub seq: u64,
    pub elapsed_us: u128,
    pub level: Level,
    pub target: String,
    pub span: Option<u64>,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:>5}] +{:>9}us {:<5} {}",
            self.seq,
            self.elapsed_us,
            self.level.as_str(),
            self.target
        )?;
        if let Some(id) = self.span {
            write!(f, " span#{}", id)?;
        }
        write!(f, ": {}", self.message)?;
        if !self.fields.is_empty() {
            write!(f, " {{")?;
            for (i, (k, v)) in self.fields.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}={}", k, v)?;
            }
            write!(f, "}}")?;
        }
        Ok(())
    }
}

const RING_CAP: usize = 16_384;

struct Sink {
    records: Vec<Record>,
}

fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn sink() -> &'static RwLock<Sink> {
    static SINK: OnceLock<RwLock<Sink>> = OnceLock::new();
    SINK.get_or_init(|| RwLock::new(Sink { records: Vec::new() }))
}

static SEQ: AtomicU64 = AtomicU64::new(0);
static SPAN_SEQ: AtomicU64 = AtomicU64::new(0);
static MIN_LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);
static TO_STDERR: AtomicBool = AtomicBool::new(false);

/// A redaction function pointer: takes the raw text and returns the redacted form.
type RedactorFn = fn(&str) -> String;

/// Optional privacy redactor applied to every message/field before a record is
/// stored or emitted. Installed by the security layer (`nexus-secure`) so logs
/// never leak PII/secrets. Kept here as a plain function pointer to avoid a
/// dependency cycle (core cannot depend on secure).
fn redactor() -> &'static RwLock<Option<RedactorFn>> {
    static R: OnceLock<RwLock<Option<RedactorFn>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(None))
}

/// Install a redaction function run on every message and field value.
pub fn set_redactor(f: fn(&str) -> String) {
    *redactor().write().unwrap() = Some(f);
}

/// Remove any installed redactor.
pub fn clear_redactor() {
    *redactor().write().unwrap() = None;
}

fn apply_redactor(s: &str) -> Option<String> {
    let guard = redactor().read().unwrap();
    guard.map(|f| f(s))
}

/// Set the minimum level that will be captured/emitted.
pub fn set_level(level: Level) {
    MIN_LEVEL.store(level as u8, Ordering::Relaxed);
}

/// Mirror captured records to stderr as they happen.
pub fn set_stderr(on: bool) {
    TO_STDERR.store(on, Ordering::Relaxed);
}

fn enabled(level: Level) -> bool {
    level as u8 >= MIN_LEVEL.load(Ordering::Relaxed)
}

/// Emit a record with explicit span and fields. The lowest-level entry point;
/// the helpers below cover the common cases.
pub fn record(level: Level, target: &str, span: Option<u64>, message: impl Into<String>, fields: &[(&str, &str)]) {
    if !enabled(level) {
        return;
    }
    let mut message = message.into();
    let mut fields: Vec<(String, String)> = fields.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    // Privacy: scrub PII/secrets out of the record if a redactor is installed.
    if let Some(red) = apply_redactor(&message) {
        message = red;
        for (_, v) in fields.iter_mut() {
            if let Some(rv) = apply_redactor(v) {
                *v = rv;
            }
        }
    }
    let rec = Record {
        seq: SEQ.fetch_add(1, Ordering::Relaxed),
        elapsed_us: start().elapsed().as_micros(),
        level,
        target: target.to_string(),
        span,
        message,
        fields,
    };
    if TO_STDERR.load(Ordering::Relaxed) {
        eprintln!("{}", rec);
    }
    let mut s = sink().write().unwrap();
    if s.records.len() >= RING_CAP {
        let drop = s.records.len() - RING_CAP + 1;
        s.records.drain(0..drop);
    }
    s.records.push(rec);
}

/// `target: message` at a level, no fields.
pub fn event(level: Level, target: &str, message: impl Into<String>) {
    record(level, target, None, message, &[]);
}

pub fn trace(target: &str, message: impl Into<String>) {
    event(Level::Trace, target, message);
}
pub fn debug(target: &str, message: impl Into<String>) {
    event(Level::Debug, target, message);
}
pub fn info(target: &str, message: impl Into<String>) {
    event(Level::Info, target, message);
}
pub fn warn(target: &str, message: impl Into<String>) {
    event(Level::Warn, target, message);
}
pub fn error(target: &str, message: impl Into<String>) {
    event(Level::Error, target, message);
}

/// An RAII span: logs an `enter` on creation and an `exit` (with duration) on
/// drop, tagging every record made with `span.record(...)` with its id so a
/// trace can be grouped into nested scopes.
pub struct Span {
    id: u64,
    target: String,
    name: String,
    began_us: u128,
}

impl Span {
    pub fn id(&self) -> u64 {
        self.id
    }
    /// Emit a record attributed to this span.
    pub fn record(&self, level: Level, message: impl Into<String>, fields: &[(&str, &str)]) {
        record(level, &self.target, Some(self.id), message, fields);
    }
    pub fn info(&self, message: impl Into<String>) {
        self.record(Level::Info, message, &[]);
    }
    pub fn debug(&self, message: impl Into<String>) {
        self.record(Level::Debug, message, &[]);
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let dur = start().elapsed().as_micros().saturating_sub(self.began_us);
        record(
            Level::Debug,
            &self.target,
            Some(self.id),
            format!("exit {}", self.name),
            &[("dur_us", &dur.to_string())],
        );
    }
}

/// Open a new span.
pub fn span(target: &str, name: impl Into<String>) -> Span {
    let id = SPAN_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = name.into();
    let began_us = start().elapsed().as_micros();
    record(Level::Debug, target, Some(id), format!("enter {}", name), &[]);
    Span {
        id,
        target: target.to_string(),
        name,
        began_us,
    }
}

/// Snapshot all currently buffered records.
pub fn snapshot() -> Vec<Record> {
    sink().read().unwrap().records.clone()
}

/// Number of buffered records.
pub fn count() -> usize {
    sink().read().unwrap().records.len()
}

/// Drop all buffered records (does not reset the sequence counter).
pub fn clear() {
    sink().write().unwrap().records.clear();
}

/// Render the buffered trace as newline-separated text.
pub fn render() -> String {
    let s = sink().read().unwrap();
    let mut out = String::new();
    for r in &s.records {
        out.push_str(&r.to_string());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The trace sink is process-global, so these tests must not interleave:
    // each clears the sink and sets the level. Serialize them with a shared
    // lock (ignoring poisoning from a panicking test).
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn levels_filter_and_capture() {
        let _g = guard();
        clear();
        set_level(Level::Debug);
        let before = count();
        trace("test", "should be dropped"); // below threshold
        debug("test", "kept");
        info("test", "also kept");
        assert_eq!(count(), before + 2);
    }

    #[test]
    fn spans_record_enter_and_exit() {
        let _g = guard();
        clear();
        set_level(Level::Trace);
        let n = count();
        {
            let sp = span("test", "work");
            sp.info("inside");
        } // exit logged on drop
        let recs = snapshot();
        assert!(recs.len() >= n + 3);
        assert!(recs.iter().any(|r| r.message.starts_with("enter work")));
        assert!(recs.iter().any(|r| r.message.starts_with("exit work")));
    }

    #[test]
    fn redactor_scrubs_records() {
        let _g = guard();
        clear();
        set_level(Level::Info);
        fn mask(s: &str) -> String {
            s.replace("secret", "[REDACTED]")
        }
        set_redactor(mask);
        record(Level::Info, "test", None, "the secret is here", &[("v", "more secret")]);
        let r = snapshot().pop().unwrap();
        clear_redactor();
        assert!(!r.message.contains("secret"), "msg: {}", r.message);
        assert!(r.message.contains("[REDACTED]"));
        assert!(r.fields[0].1.contains("[REDACTED]"));
    }

    #[test]
    fn fields_render() {
        let _g = guard();
        clear();
        set_level(Level::Info);
        record(Level::Info, "test", None, "dispatch", &[("backend", "simd"), ("n", "8")]);
        let r = snapshot().pop().unwrap();
        assert!(r.to_string().contains("backend=simd"));
        assert!(r.to_string().contains("n=8"));
    }
}
