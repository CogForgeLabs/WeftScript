//! SECURITY — a lightweight capability sandbox for shell commands and network
//! hosts.
//!
//! A [`Sandbox`] is permissive by default except for a built-in denylist of
//! obviously destructive shell patterns that are *always* refused. Operators
//! can tighten it via environment variables or builder methods: setting an
//! allowlist switches the relevant dimension into "deny everything not listed"
//! mode.

use nexus_core::trace::{record, Level};

/// Shell fragments that are always denied regardless of configuration.
const DESTRUCTIVE: &[&str] = &[
    "rm -rf /",
    "rm -fr /",
    "mkfs",
    "dd if=",
    ":(){",
    "shutdown",
    "format ",
];

/// A capability sandbox over commands and hosts.
#[derive(Clone, Debug, Default)]
pub struct Sandbox {
    /// If `Some`, only these program basenames may run (allowlist mode).
    allow_cmds: Option<Vec<String>>,
    deny_cmds: Vec<String>,
    /// If `Some`, only these hosts may be contacted (allowlist mode).
    allow_hosts: Option<Vec<String>>,
    deny_hosts: Vec<String>,
}

/// Reduce a program string to its basename (strip directory components and any
/// surrounding quotes).
fn basename(prog: &str) -> String {
    let p = prog.trim().trim_matches('"').trim_matches('\'');
    let p = p.rsplit(['/', '\\']).next().unwrap_or(p);
    p.to_string()
}

/// Extract the bare host from a "host", "host:port", or URL form.
fn extract_host(input: &str) -> String {
    let mut s = input.trim();
    // Strip scheme.
    if let Some(idx) = s.find("://") {
        s = &s[idx + 3..];
    }
    // Strip path / query / fragment.
    for sep in ['/', '?', '#'] {
        if let Some(idx) = s.find(sep) {
            s = &s[..idx];
        }
    }
    // Strip userinfo.
    if let Some(idx) = s.rfind('@') {
        s = &s[idx + 1..];
    }
    // Strip port (only when it is not part of an unbracketed IPv6 literal).
    if !s.contains(']') {
        if let Some(idx) = s.rfind(':') {
            if s[idx + 1..].chars().all(|c| c.is_ascii_digit()) {
                s = &s[..idx];
            }
        }
    }
    s.trim_matches(['[', ']']).to_ascii_lowercase()
}

/// `needle` matches `host` exactly or as a parent domain (so `evil.com` also
/// matches `sub.evil.com`).
fn host_matches(host: &str, needle: &str) -> bool {
    host == needle || host.ends_with(&format!(".{needle}"))
}

fn parse_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Split a shell command line into the individual program invocations it will
/// run, so each can be vetted. This is a best-effort lexical split on the common
/// separators (`;`, `&&`, `||`, `|`, newlines); it is deliberately conservative
/// and pairs with [`uses_command_substitution`] to refuse constructs it cannot
/// split safely.
fn command_segments(cmd: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let bytes = cmd.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let two = cmd.get(i..i + 2);
        if matches!(two, Some("&&") | Some("||")) {
            segments.push(std::mem::take(&mut current));
            i += 2;
            continue;
        }
        let c = bytes[i] as char;
        if c == ';' || c == '|' || c == '\n' || c == '\r' || c == '&' {
            segments.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(c);
        i += 1;
    }
    segments.push(current);
    segments.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Whether the command uses command substitution or a subshell, which hides the
/// spawned program from a lexical allowlist check.
fn uses_command_substitution(cmd: &str) -> bool {
    cmd.contains("$(") || cmd.contains('`') || cmd.contains(">(") || cmd.contains("<(")
}

impl Sandbox {
    /// Permissive sandbox: everything is allowed except the built-in
    /// destructive denylist.
    pub fn new() -> Sandbox {
        Sandbox::default()
    }

    /// Build a sandbox from environment configuration:
    /// `NEXUS_ALLOW_CMDS`, `NEXUS_DENY_CMDS`, `NEXUS_ALLOW_HOSTS`,
    /// `NEXUS_DENY_HOSTS` (all comma-separated).
    pub fn from_env() -> Sandbox {
        let mut sb = Sandbox::new();
        if let Ok(v) = std::env::var("NEXUS_ALLOW_CMDS") {
            let list = parse_csv(&v);
            if !list.is_empty() {
                sb.allow_cmds = Some(list.iter().map(|p| basename(p)).collect());
            }
        }
        if let Ok(v) = std::env::var("NEXUS_DENY_CMDS") {
            sb.deny_cmds = parse_csv(&v).iter().map(|p| basename(p)).collect();
        }
        if let Ok(v) = std::env::var("NEXUS_ALLOW_HOSTS") {
            let list = parse_csv(&v);
            if !list.is_empty() {
                sb.allow_hosts = Some(list.iter().map(|h| extract_host(h)).collect());
            }
        }
        if let Ok(v) = std::env::var("NEXUS_DENY_HOSTS") {
            sb.deny_hosts = parse_csv(&v).iter().map(|h| extract_host(h)).collect();
        }
        sb
    }

    /// Add a program to the allowlist (switching command checks into allowlist
    /// mode if not already).
    pub fn allow_command(&mut self, prog: &str) -> &mut Self {
        self.allow_cmds.get_or_insert_with(Vec::new).push(basename(prog));
        self
    }

    /// Add a program to the denylist.
    pub fn deny_command(&mut self, prog: &str) -> &mut Self {
        self.deny_cmds.push(basename(prog));
        self
    }

    /// Add a host to the allowlist (switching host checks into allowlist mode).
    pub fn allow_host(&mut self, host: &str) -> &mut Self {
        self.allow_hosts.get_or_insert_with(Vec::new).push(extract_host(host));
        self
    }

    /// Add a host to the denylist.
    pub fn deny_host(&mut self, host: &str) -> &mut Self {
        self.deny_hosts.push(extract_host(host));
        self
    }

    /// Check whether a shell command may run.
    ///
    /// The command is executed through `sh -c` / `cmd /C`, so a single string can
    /// invoke several programs via `;`, `&&`, `||`, `|`, or newlines. Every
    /// segment is vetted, not just the first token — otherwise `echo x; curl evil`
    /// would sail past an allowlist that only listed `echo`.
    pub fn check_command(&self, cmd: &str) -> Result<(), String> {
        let trimmed = cmd.trim();

        // 1. Built-in destructive denylist (always enforced, whole string).
        for pat in DESTRUCTIVE {
            if trimmed.contains(pat) {
                let why = format!("command denied: matches destructive pattern {pat:?}");
                record(Level::Warn, "secure.sandbox", None, why.clone(), &[("cmd", trimmed)]);
                return Err(why);
            }
        }

        // 2. When an allowlist is active, command substitution / subshells hide
        //    the spawned program from the lexical check, so refuse them outright.
        let allowlist_active = self.allow_cmds.is_some();
        if allowlist_active && uses_command_substitution(trimmed) {
            let why = "command denied: command substitution/subshell not permitted under an allowlist".to_string();
            record(Level::Warn, "secure.sandbox", None, why.clone(), &[("cmd", trimmed)]);
            return Err(why);
        }

        // 3. Vet every program the command line will invoke.
        let segments = command_segments(trimmed);
        let progs: Vec<String> = if segments.is_empty() {
            vec![basename(trimmed.split_whitespace().next().unwrap_or(""))]
        } else {
            segments.iter().map(|s| basename(s.split_whitespace().next().unwrap_or(""))).collect()
        };

        for prog in &progs {
            // 3a. Explicit denylist.
            if self.deny_cmds.iter().any(|d| d == prog) {
                let why = format!("command denied: {prog:?} is on the denylist");
                record(Level::Warn, "secure.sandbox", None, why.clone(), &[("prog", prog)]);
                return Err(why);
            }
            // 3b. Allowlist (if configured, only listed programs pass).
            if let Some(allow) = &self.allow_cmds {
                if !allow.iter().any(|a| a == prog) {
                    let why = format!("command denied: {prog:?} is not on the allowlist");
                    record(Level::Warn, "secure.sandbox", None, why.clone(), &[("prog", prog)]);
                    return Err(why);
                }
            }
        }

        record(Level::Debug, "secure.sandbox", None, "command allowed", &[("progs", &progs.join(","))]);
        Ok(())
    }

    /// Check whether a host may be contacted. Accepts "host", "host:port", or a
    /// URL.
    pub fn check_host(&self, host: &str) -> Result<(), String> {
        let h = extract_host(host);
        if h.is_empty() {
            let why = "host denied: empty host".to_string();
            record(Level::Warn, "secure.sandbox", None, why.clone(), &[("host", host)]);
            return Err(why);
        }

        // 1. Denylist.
        if self.deny_hosts.iter().any(|d| host_matches(&h, d)) {
            let why = format!("host denied: {h:?} is on the denylist");
            record(Level::Warn, "secure.sandbox", None, why.clone(), &[("host", &h)]);
            return Err(why);
        }

        // 2. Allowlist.
        if let Some(allow) = &self.allow_hosts {
            if !allow.iter().any(|a| host_matches(&h, a)) {
                let why = format!("host denied: {h:?} is not on the allowlist");
                record(Level::Warn, "secure.sandbox", None, why.clone(), &[("host", &h)]);
                return Err(why);
            }
        }

        record(Level::Debug, "secure.sandbox", None, "host allowed", &[("host", &h)]);
        Ok(())
    }
}

/// Convenience trace redactor: scrubs PII / secrets before they reach a log.
pub fn redact_secrets_for_log(s: &str) -> String {
    crate::redact::redact(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_always_denied() {
        let sb = Sandbox::new();
        assert!(sb.check_command("rm -rf /").is_err());
        assert!(sb.check_command("dd if=/dev/zero of=/dev/sda").is_err());
        assert!(sb.check_command("mkfs.ext4 /dev/sda1").is_err());
        assert!(sb.check_command("shutdown -h now").is_err());
    }

    #[test]
    fn permissive_by_default() {
        let sb = Sandbox::new();
        assert!(sb.check_command("echo hi").is_ok());
        assert!(sb.check_command("/usr/bin/python script.py").is_ok());
    }

    #[test]
    fn allowlist_mode_only_listed_pass() {
        let mut sb = Sandbox::new();
        sb.allow_command("echo");
        assert!(sb.check_command("echo hi").is_ok());
        assert!(sb.check_command("cat file").is_err());
        // even allowlisted programs cannot bypass the destructive denylist
        sb.allow_command("rm");
        assert!(sb.check_command("rm -rf /").is_err());
    }

    #[test]
    fn denylist_blocks() {
        let mut sb = Sandbox::new();
        sb.deny_command("curl");
        assert!(sb.check_command("curl http://x").is_err());
        assert!(sb.check_command("echo ok").is_ok());
    }

    #[test]
    fn chained_commands_cannot_bypass_allowlist() {
        // The command runs through `sh -c`, so every segment must be vetted, not
        // just the first token.
        let mut sb = Sandbox::new();
        sb.allow_command("echo");
        assert!(sb.check_command("echo hi").is_ok());
        // Each of these smuggles a second, non-allowlisted program.
        assert!(sb.check_command("echo hi; curl evil.com").is_err());
        assert!(sb.check_command("echo hi && rm file").is_err());
        assert!(sb.check_command("echo hi || wget x").is_err());
        assert!(sb.check_command("echo hi | sh").is_err());
        assert!(sb.check_command("echo a & nc -l 1234").is_err());
        assert!(sb.check_command("echo one\ncurl evil.com").is_err());
    }

    #[test]
    fn chained_commands_vetted_against_denylist() {
        let mut sb = Sandbox::new();
        sb.deny_command("curl");
        // curl hidden after a separator is still caught.
        assert!(sb.check_command("echo hi; curl evil.com").is_err());
        assert!(sb.check_command("ls && curl x | grep y").is_err());
        assert!(sb.check_command("ls -la").is_ok());
    }

    #[test]
    fn command_substitution_refused_under_allowlist() {
        let mut sb = Sandbox::new();
        sb.allow_command("echo");
        assert!(sb.check_command("echo $(rm -rf data)").is_err());
        assert!(sb.check_command("echo `whoami`").is_err());
        // Without an allowlist, substitution is permitted (permissive default),
        // but the destructive denylist still applies to the whole string.
        let permissive = Sandbox::new();
        assert!(permissive.check_command("echo $(date)").is_ok());
    }

    #[test]
    fn host_deny_works() {
        let mut sb = Sandbox::new();
        sb.deny_host("evil.com");
        assert!(sb.check_host("evil.com").is_err());
        assert!(sb.check_host("sub.evil.com").is_err());
        assert!(sb.check_host("https://evil.com:443/path").is_err());
        assert!(sb.check_host("good.com").is_ok());
    }

    #[test]
    fn host_allowlist_works() {
        let mut sb = Sandbox::new();
        sb.allow_host("api.example.com");
        assert!(sb.check_host("api.example.com:443").is_ok());
        assert!(sb.check_host("other.com").is_err());
    }

    #[test]
    fn extract_host_forms() {
        assert_eq!(extract_host("host:8080"), "host");
        assert_eq!(extract_host("https://user@Example.COM:443/p?q=1"), "example.com");
        assert_eq!(extract_host("plainhost"), "plainhost");
    }
}
