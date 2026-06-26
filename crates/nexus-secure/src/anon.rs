//! ANONYMITY — pseudonyms, ephemeral tokens, and identity-stripping helpers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Stable pseudonym for an input. The same input always yields the same
/// pseudonym; different inputs yield different ones. Format: `anon_<12 hex>`.
pub fn anonymize(s: &str) -> String {
    let hash = blake3::hash(s.as_bytes());
    let hex = hash.to_hex();
    format!("anon_{}", &hex.as_str()[..12])
}

/// Ephemeral, hard-to-guess token. Entropy is derived from the current time in
/// nanoseconds, a process-global monotonic counter, and the process id — no
/// `rand` dependency. Two calls always differ. Format: `tok_<16 hex>`.
pub fn anon_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id() as u64;
    // The counter guarantees a distinct input on every call even if the clock
    // and pid are identical.
    let mix = nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ pid;
    let hash = blake3::hash(&mix.to_le_bytes());
    let hex = hash.to_hex();
    format!("tok_{}", &hex.as_str()[..16])
}

/// Generic identity-stripping HTTP headers: a neutral User-Agent, Do-Not-Track,
/// and no cookies or referer.
pub fn anonymous_headers() -> Vec<(String, String)> {
    vec![
        ("User-Agent".to_string(), "Nexus/1.0".to_string()),
        ("DNT".to_string(), "1".to_string()),
        ("Accept".to_string(), "*/*".to_string()),
        ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
    ]
}

/// Optional outbound proxy (e.g. a Tor SOCKS endpoint like `127.0.0.1:9050`)
/// read from `NEXUS_PROXY`. Routing is wired up elsewhere; this just exposes it.
pub fn proxy_from_env() -> Option<String> {
    match std::env::var("NEXUS_PROXY") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
}

/// Re-export of [`crate::redact::redact`] for symmetry with the privacy module.
pub fn scrub_pii(text: &str) -> String {
    crate::redact::redact(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymize_is_stable_and_distinct() {
        let a1 = anonymize("alice@example.com");
        let a2 = anonymize("alice@example.com");
        let b = anonymize("bob@example.com");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert!(a1.starts_with("anon_"));
        assert_eq!(a1.len(), "anon_".len() + 12);
    }

    #[test]
    fn anon_token_differs_each_call() {
        let t1 = anon_token();
        let t2 = anon_token();
        assert_ne!(t1, t2);
        assert!(t1.starts_with("tok_"));
        assert_eq!(t1.len(), "tok_".len() + 16);
    }

    #[test]
    fn headers_strip_identity() {
        let h = anonymous_headers();
        assert!(h.iter().any(|(k, v)| k == "User-Agent" && v == "Nexus/1.0"));
        assert!(!h.iter().any(|(k, _)| k.eq_ignore_ascii_case("Cookie")));
        assert!(!h.iter().any(|(k, _)| k.eq_ignore_ascii_case("Referer")));
        assert!(h.iter().any(|(k, _)| k == "DNT"));
    }

    #[test]
    fn proxy_env_optional() {
        // Without the var set in this process it should be None (or whatever the
        // environment provides); just ensure it does not panic.
        let _ = proxy_from_env();
    }

    #[test]
    fn scrub_pii_redacts() {
        assert!(scrub_pii("mail alice@example.com").contains("[REDACTED:email]"));
    }
}
