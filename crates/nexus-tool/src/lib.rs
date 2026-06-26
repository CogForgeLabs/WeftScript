//! # nexus-tool
//!
//! Terminal / CLI / software automation for Nexus. This is what lets a Nexus
//! program *drive other tools* — run shell commands, generate and mutate
//! command lines, auto-correct a failing invocation by trying alternatives,
//! offload independent commands across threads, and enforce a wall-clock
//! budget per command. Every action is emitted through `nexus_core::trace` so a
//! whole automation run can be replayed end to end.
//!
//! The design goal is *declarative tool use*: you describe a command template
//! and parameters (or a list of candidate fixes) and the runner figures out the
//! concrete invocation, runs it, and reports a structured result.

pub mod net;

use nexus_core::trace::{self, Level};
use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Structured result of running a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CmdResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// True if the command exited 0 within its time budget.
    pub ok: bool,
    /// True if the command was killed for exceeding its time budget.
    pub timed_out: bool,
}

impl CmdResult {
    fn failure(msg: impl Into<String>) -> CmdResult {
        CmdResult { code: -1, stdout: String::new(), stderr: msg.into(), ok: false, timed_out: false }
    }
}

/// Build a shell command for the host platform.
fn shell(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    }
}

/// Run a shell command to completion, capturing stdout/stderr.
pub fn run(cmd: &str) -> CmdResult {
    run_timeout(cmd, None)
}

/// Run a shell command with an optional wall-clock budget (milliseconds). On
/// timeout the child is killed and `timed_out` is set. The whole thing is
/// traced: an `enter`/`exit` span plus the exit code.
pub fn run_timeout(cmd: &str, timeout_ms: Option<u64>) -> CmdResult {
    let sp = trace::span("tool.run", format!("$ {}", truncate(cmd, 120)));
    let started = Instant::now();
    let mut child = match shell(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            sp.record(Level::Error, "spawn failed", &[("err", &e.to_string())]);
            return CmdResult::failure(format!("spawn failed: {e}"));
        }
    };

    // Drain stdout/stderr on threads so a chatty child can't deadlock on a full
    // pipe while we wait for it.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err_handle = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if let Some(limit) = timeout_ms {
                    if started.elapsed() >= Duration::from_millis(limit) {
                        let _ = child.kill();
                        let st = child.wait().ok();
                        timed_out = true;
                        break st;
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break None,
        }
    };

    let stdout = out_handle.join().unwrap_or_default();
    let stderr = err_handle.join().unwrap_or_default();
    let code = status.and_then(|s| s.code()).unwrap_or(-1);
    let ok = !timed_out && code == 0;
    sp.record(
        if ok { Level::Info } else { Level::Warn },
        "command finished",
        &[("code", &code.to_string()), ("timed_out", &timed_out.to_string()), ("ms", &started.elapsed().as_millis().to_string())],
    );
    CmdResult { code, stdout, stderr, ok, timed_out }
}

/// Locate an executable on PATH (declarative capability probing).
pub fn which(name: &str) -> Option<String> {
    let probe = if cfg!(windows) {
        format!("where {}", name)
    } else {
        format!("command -v {}", name)
    };
    let r = run(&probe);
    if r.ok {
        r.stdout.lines().next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Fill a `<key>` template from a parameter map — the declarative command
/// generator. Angle brackets are used (not `{}`) so a deferred template never
/// collides with the Nexus language's own `{}` string interpolation. Unknown
/// placeholders are left intact so the caller can detect them.
pub fn render_template(template: &str, params: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c == '<' {
            let mut key = String::new();
            let mut closed = false;
            while let Some(&(_, n)) = chars.peek() {
                chars.next();
                if n == '>' {
                    closed = true;
                    break;
                }
                key.push(n);
            }
            // Only treat `<ident>` as a placeholder; anything else (e.g. a shell
            // redirect `<`) is passed through verbatim.
            let is_ident = !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_');
            if closed && is_ident {
                match params.get(&key) {
                    Some(v) => out.push_str(v),
                    None => {
                        out.push('<');
                        out.push_str(&key);
                        out.push('>');
                    }
                }
            } else {
                out.push('<');
                out.push_str(&key);
                if closed {
                    out.push('>');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A small dictionary of well-known commands used for typo auto-correction.
const KNOWN_COMMANDS: &[&str] = &[
    "echo", "git", "cargo", "python", "python3", "node", "npm", "ls", "dir", "cat", "type", "grep",
    "find", "where", "rustc", "pip", "docker", "kubectl", "curl", "make", "go",
];

/// Levenshtein edit distance (small inputs, so the simple DP is fine).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Generate corrected/mutated variants of a command line, best-first. The first
/// token (the program) is matched against [`KNOWN_COMMANDS`]; close typos
/// produce a corrected candidate. This is the "command gen + mutation" stage
/// that feeds [`auto_fix`].
pub fn mutate(cmd: &str) -> Vec<String> {
    let trimmed = cmd.trim();
    let mut variants = Vec::new();
    let (prog, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((p, r)) => (p, r),
        None => (trimmed, ""),
    };
    if !KNOWN_COMMANDS.contains(&prog) {
        // Find known commands within edit distance 1..=2, closest first.
        let mut cands: Vec<(usize, &str)> = KNOWN_COMMANDS
            .iter()
            .map(|k| (edit_distance(prog, k), *k))
            .filter(|(d, _)| *d >= 1 && *d <= 2)
            .collect();
        cands.sort_by_key(|(d, _)| *d);
        for (_, k) in cands {
            let v = if rest.is_empty() { k.to_string() } else { format!("{} {}", k, rest) };
            if !variants.contains(&v) {
                variants.push(v);
            }
        }
    }
    trace::record(
        Level::Debug,
        "tool.mutate",
        None,
        "generated candidates",
        &[("program", prog), ("count", &variants.len().to_string())],
    );
    variants
}

/// Run candidate commands in order, returning the first that succeeds (exit 0)
/// together with its index, otherwise the last result. This is the
/// auto-debugging / self-correcting retry loop: each attempt is traced.
pub fn auto_run(candidates: &[String]) -> (usize, CmdResult) {
    let sp = trace::span("tool.auto_run", format!("{} candidate(s)", candidates.len()));
    let mut last = CmdResult::failure("no candidates");
    for (i, c) in candidates.iter().enumerate() {
        sp.record(Level::Debug, "attempt", &[("i", &i.to_string()), ("cmd", &truncate(c, 100))]);
        let r = run(c);
        if r.ok {
            sp.record(Level::Info, "succeeded", &[("i", &i.to_string())]);
            return (i, r);
        }
        last = r;
    }
    sp.record(Level::Warn, "all candidates failed", &[]);
    (candidates.len().saturating_sub(1), last)
}

/// Try a command; if it fails, generate corrected variants and try those. The
/// declarative "just make it work" entry point — returns the successful result
/// and the exact command that worked.
pub fn auto_fix(cmd: &str) -> (String, CmdResult) {
    let first = run(cmd);
    if first.ok {
        return (cmd.to_string(), first);
    }
    let variants = mutate(cmd);
    if variants.is_empty() {
        return (cmd.to_string(), first);
    }
    let (idx, res) = auto_run(&variants);
    if res.ok {
        (variants[idx].clone(), res)
    } else {
        (cmd.to_string(), first)
    }
}

/// Run independent commands concurrently (auto-offload / parallelize), results
/// returned in input order.
pub fn run_parallel(cmds: &[String]) -> Vec<CmdResult> {
    if cmds.is_empty() {
        return Vec::new();
    }
    trace::record(Level::Debug, "tool.parallel", None, "offload", &[("cmds", &cmds.len().to_string())]);
    let handles: Vec<_> = cmds
        .iter()
        .cloned()
        .map(|c| std::thread::spawn(move || run(&c)))
        .collect();
    handles.into_iter().map(|h| h.join().unwrap_or_else(|_| CmdResult::failure("worker panicked"))).collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{}…", head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_cmd(text: &str) -> String {
        // `echo` works on both cmd.exe and sh.
        format!("echo {}", text)
    }

    #[test]
    fn runs_a_command_and_captures_output() {
        let r = run(&echo_cmd("hello"));
        assert!(r.ok, "echo should succeed: {:?}", r);
        assert!(r.stdout.contains("hello"));
        assert_eq!(r.code, 0);
    }

    #[test]
    fn nonzero_exit_is_not_ok() {
        let r = run("exit 3");
        assert!(!r.ok);
        assert_eq!(r.code, 3);
    }

    #[test]
    fn timeout_kills_long_command() {
        // A command that would sleep far longer than the budget.
        let cmd = if cfg!(windows) {
            // ping with a delay is the portable "sleep" on stock Windows.
            "ping -n 10 127.0.0.1"
        } else {
            "sleep 10"
        };
        let r = run_timeout(cmd, Some(150));
        assert!(r.timed_out, "expected timeout, got {:?}", r);
        assert!(!r.ok);
    }

    #[test]
    fn template_rendering() {
        let mut p = BTreeMap::new();
        p.insert("sub".to_string(), "commit".to_string());
        p.insert("msg".to_string(), "hi".to_string());
        assert_eq!(render_template("git <sub> -m <msg>", &p), "git commit -m hi");
        // Unknown placeholder is preserved.
        assert_eq!(render_template("a <x> b", &p), "a <x> b");
    }

    #[test]
    fn mutate_corrects_typos() {
        let v = mutate("ecHo hi"); // wrong case -> not in dict; closest is "echo"
        assert!(v.iter().any(|c| c.starts_with("echo ")), "got {:?}", v);
        let v2 = mutate("gti status");
        assert!(v2.iter().any(|c| c.starts_with("git ")), "got {:?}", v2);
    }

    #[test]
    fn auto_fix_recovers_from_a_typo() {
        // `ecHo` fails on sh (case-sensitive) / behaves oddly; the corrected
        // `echo` candidate should succeed. On Windows cmd is case-insensitive so
        // the first try already works — either way auto_fix returns success.
        let (_winner, res) = auto_fix("ecHo recovered");
        assert!(res.ok, "auto_fix should produce a working command: {:?}", res);
        assert!(res.stdout.to_lowercase().contains("recovered"));
    }

    #[test]
    fn auto_run_returns_first_success() {
        let cands = vec!["exit 1".to_string(), echo_cmd("won"), echo_cmd("later")];
        let (idx, res) = auto_run(&cands);
        assert_eq!(idx, 1);
        assert!(res.ok && res.stdout.contains("won"));
    }

    #[test]
    fn parallel_runs_all() {
        let cmds = vec![echo_cmd("a"), echo_cmd("b"), echo_cmd("c")];
        let results = run_parallel(&cmds);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.ok));
    }
}
