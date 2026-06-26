//! PRIVACY — PII / secret detection and redaction.
//!
//! All detectors are implemented by hand (character / byte scanning); there is
//! no regex crate in the workspace. Detected sensitive substrings are replaced
//! with `[REDACTED:<kind>]`. When candidate matches overlap, the more specific
//! (and longer) match wins so a span is never redacted twice.

/// A detected sensitive span over the input bytes.
#[derive(Clone, Debug)]
struct Match {
    start: usize,
    end: usize,
    kind: &'static str,
    /// Higher priority wins when two candidate spans overlap.
    prio: u8,
}

#[inline]
fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}
#[inline]
fn is_alpha(b: u8) -> bool {
    b.is_ascii_alphabetic()
}
#[inline]
fn is_email_local(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-')
}
#[inline]
fn is_domain_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')
}
#[inline]
fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
}

/// Luhn checksum validator. Non-digit characters are ignored. Empty input (no
/// digits) is invalid.
pub fn luhn_valid(digits: &str) -> bool {
    let nums: Vec<u8> = digits.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    if nums.is_empty() {
        return false;
    }
    let mut sum: u32 = 0;
    // Double every second digit starting from the rightmost.
    for (i, &d) in nums.iter().rev().enumerate() {
        let mut v = d as u32;
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum % 10 == 0
}

fn detect_emails(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    let mut i = 0;
    while i < len {
        if b[i] == b'@' {
            // Expand left over the local part.
            let mut start = i;
            while start > 0 && is_email_local(b[start - 1]) {
                start -= 1;
            }
            // Expand right over the domain.
            let mut end = i + 1;
            while end < len && is_domain_char(b[end]) {
                end += 1;
            }
            // Trim trailing separators (e.g. a sentence period after the addr).
            while end > i + 1 && matches!(b[end - 1], b'.' | b'-') {
                end -= 1;
            }
            let local = &b[start..i];
            let domain = &b[i + 1..end];
            if !local.is_empty() && valid_domain(domain) {
                out.push(Match { start, end, kind: "email", prio: 5 });
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

fn valid_domain(domain: &[u8]) -> bool {
    if domain.is_empty() || domain[0] == b'.' {
        return false;
    }
    // Must contain a dot and a TLD of >= 2 alphabetic chars.
    let dot = match domain.iter().rposition(|&c| c == b'.') {
        Some(p) => p,
        None => return false,
    };
    let tld = &domain[dot + 1..];
    tld.len() >= 2 && tld.iter().all(|&c| is_alpha(c))
}

fn detect_credit_cards(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    let mut i = 0;
    while i < len {
        if is_digit(b[i]) && (i == 0 || !is_digit(b[i - 1])) {
            // Walk a run of digits joined by single space/dash separators.
            let start = i;
            let mut end = i;
            let mut last_digit = i;
            let mut digit_count = 0usize;
            let mut j = i;
            while j < len {
                if is_digit(b[j]) {
                    digit_count += 1;
                    last_digit = j;
                    end = j + 1;
                    j += 1;
                } else if matches!(b[j], b' ' | b'-') && j + 1 < len && is_digit(b[j + 1]) {
                    // separator between two digits — keep going
                    j += 1;
                } else {
                    break;
                }
            }
            let span_end = last_digit + 1;
            if (13..=19).contains(&digit_count) {
                let slice = std::str::from_utf8(&b[start..span_end]).unwrap_or("");
                if luhn_valid(slice) {
                    out.push(Match { start, end: span_end, kind: "credit_card", prio: 5 });
                }
            }
            let _ = end;
            i = span_end.max(i + 1);
            continue;
        }
        i += 1;
    }
}

fn detect_ssns(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    if len < 11 {
        return;
    }
    let mut i = 0;
    while i + 11 <= len {
        let ok = is_digit(b[i]) && is_digit(b[i + 1]) && is_digit(b[i + 2])
            && b[i + 3] == b'-'
            && is_digit(b[i + 4]) && is_digit(b[i + 5])
            && b[i + 6] == b'-'
            && is_digit(b[i + 7]) && is_digit(b[i + 8]) && is_digit(b[i + 9]) && is_digit(b[i + 10]);
        if ok {
            let before_ok = i == 0 || !is_digit(b[i - 1]);
            let after_ok = i + 11 == len || (!is_digit(b[i + 11]) && b[i + 11] != b'-');
            if before_ok && after_ok {
                out.push(Match { start: i, end: i + 11, kind: "ssn", prio: 5 });
                i += 11;
                continue;
            }
        }
        i += 1;
    }
}

fn parse_octet(b: &[u8], pos: usize) -> Option<usize> {
    let len = b.len();
    let mut end = pos;
    while end < len && is_digit(b[end]) && end - pos < 3 {
        end += 1;
    }
    if end == pos {
        return None;
    }
    let val: u32 = std::str::from_utf8(&b[pos..end]).ok()?.parse().ok()?;
    if val <= 255 {
        Some(end)
    } else {
        None
    }
}

fn detect_ipv4(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    let mut i = 0;
    while i < len {
        if is_digit(b[i]) && (i == 0 || (!is_digit(b[i - 1]) && b[i - 1] != b'.')) {
            if let Some(end) = match_ipv4(b, i) {
                let after_ok = end == len || (!is_digit(b[end]) && b[end] != b'.');
                if after_ok {
                    out.push(Match { start: i, end, kind: "ipv4", prio: 5 });
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn match_ipv4(b: &[u8], start: usize) -> Option<usize> {
    let len = b.len();
    let mut pos = parse_octet(b, start)?;
    for _ in 0..3 {
        if pos >= len || b[pos] != b'.' {
            return None;
        }
        pos += 1;
        pos = parse_octet(b, pos)?;
    }
    Some(pos)
}

fn detect_phones(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    let mut i = 0;
    while i < len {
        let starter = b[i] == b'+' || b[i] == b'(' || is_digit(b[i]);
        if starter && (i == 0 || (!is_digit(b[i - 1]) && b[i - 1] != b'+')) {
            if let Some(end) = match_phone(b, i) {
                let after_ok = end == len || !is_digit(b[end]);
                if after_ok {
                    out.push(Match { start: i, end, kind: "phone", prio: 3 });
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// Conservative North-American-style phone matcher. Requires explicit
/// separators (space / dash / parens) so plain numbers are not eaten.
fn match_phone(b: &[u8], start: usize) -> Option<usize> {
    let len = b.len();
    let mut pos = start;

    // Optional country code: '+' then 1-3 digits then a separator.
    if pos < len && b[pos] == b'+' {
        pos += 1;
        let cc_start = pos;
        while pos < len && is_digit(b[pos]) && pos - cc_start < 3 {
            pos += 1;
        }
        if pos == cc_start {
            return None;
        }
        if pos < len && matches!(b[pos], b' ' | b'-') {
            pos += 1;
        } else {
            return None;
        }
    }

    // Area code: "(ddd)" or "ddd".
    if pos < len && b[pos] == b'(' {
        pos += 1;
        if !take_n_digits(b, &mut pos, 3) {
            return None;
        }
        if pos < len && b[pos] == b')' {
            pos += 1;
        } else {
            return None;
        }
    } else if !take_n_digits(b, &mut pos, 3) {
        return None;
    }

    // Separator, 3 digits, separator, 4 digits.
    if !take_sep(b, &mut pos) {
        return None;
    }
    if !take_n_digits(b, &mut pos, 3) {
        return None;
    }
    if !take_sep(b, &mut pos) {
        return None;
    }
    if !take_n_digits(b, &mut pos, 4) {
        return None;
    }
    Some(pos)
}

fn take_n_digits(b: &[u8], pos: &mut usize, n: usize) -> bool {
    if *pos + n > b.len() {
        return false;
    }
    for k in 0..n {
        if !is_digit(b[*pos + k]) {
            return false;
        }
    }
    // The (n+1)-th char must not be a digit, so groups are exact.
    if *pos + n < b.len() && is_digit(b[*pos + n]) {
        return false;
    }
    *pos += n;
    true
}

fn take_sep(b: &[u8], pos: &mut usize) -> bool {
    if *pos < b.len() && matches!(b[*pos], b' ' | b'-') {
        *pos += 1;
        true
    } else {
        false
    }
}

fn detect_api_keys(b: &[u8], out: &mut Vec<Match>) {
    let len = b.len();
    let mut i = 0;
    while i < len {
        if is_token_char(b[i]) && (i == 0 || !is_token_char(b[i - 1])) {
            let start = i;
            let mut end = i;
            while end < len && is_token_char(b[end]) {
                end += 1;
            }
            let tok = &b[start..end];
            let has_alpha = tok.iter().any(|&c| is_alpha(c));
            let has_digit = tok.iter().any(|&c| is_digit(c));
            let starts_sk = tok.starts_with(b"sk-");
            let starts_akia = tok.starts_with(b"AKIA");
            let qualifies = ((starts_sk || starts_akia) && tok.len() >= 8 && has_alpha && has_digit)
                || (tok.len() >= 20 && has_alpha && has_digit);
            if qualifies {
                out.push(Match { start, end, kind: "api_key", prio: 1 });
            }
            i = end.max(i + 1);
            continue;
        }
        i += 1;
    }
}

/// Run every detector and resolve overlaps, returning the accepted spans sorted
/// by start offset.
fn find_matches(text: &str) -> Vec<Match> {
    let b = text.as_bytes();
    let mut cands: Vec<Match> = Vec::new();
    detect_emails(b, &mut cands);
    detect_credit_cards(b, &mut cands);
    detect_ssns(b, &mut cands);
    detect_ipv4(b, &mut cands);
    detect_phones(b, &mut cands);
    detect_api_keys(b, &mut cands);

    // Greedy selection: higher priority first, then longer, then earlier.
    cands.sort_by(|x, y| {
        y.prio
            .cmp(&x.prio)
            .then((y.end - y.start).cmp(&(x.end - x.start)))
            .then(x.start.cmp(&y.start))
    });

    let mut accepted: Vec<Match> = Vec::new();
    for c in cands {
        let overlaps = accepted.iter().any(|a| c.start < a.end && a.start < c.end);
        if !overlaps {
            accepted.push(c);
        }
    }
    accepted.sort_by_key(|m| m.start);
    accepted
}

/// Replace every detected sensitive substring with `[REDACTED:<kind>]`.
pub fn redact(text: &str) -> String {
    let matches = find_matches(text);
    if matches.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in &matches {
        out.push_str(&text[last..m.start]);
        out.push_str("[REDACTED:");
        out.push_str(m.kind);
        out.push(']');
        last = m.end;
    }
    out.push_str(&text[last..]);
    out
}

/// True if the text contains any detectable PII / secret.
pub fn contains_pii(text: &str) -> bool {
    !find_matches(text).is_empty()
}

/// Sorted, de-duplicated list of the kinds of PII found.
pub fn pii_kinds(text: &str) -> Vec<String> {
    let mut kinds: Vec<String> = find_matches(text).into_iter().map(|m| m.kind.to_string()).collect();
    kinds.sort();
    kinds.dedup();
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email() {
        let r = redact("contact alice@example.com please");
        assert!(r.contains("[REDACTED:email]"));
        assert!(!r.contains("alice@example.com"));
    }

    #[test]
    fn redacts_luhn_valid_card() {
        let r = redact("card 4111 1111 1111 1111 here");
        assert!(r.contains("[REDACTED:credit_card]"), "got: {r}");
        assert!(!r.contains("4111"));
    }

    #[test]
    fn redacts_ssn() {
        let r = redact("ssn 123-45-6789 done");
        assert!(r.contains("[REDACTED:ssn]"), "got: {r}");
    }

    #[test]
    fn redacts_ipv4() {
        let r = redact("server at 192.168.0.1 listening");
        assert!(r.contains("[REDACTED:ipv4]"), "got: {r}");
        assert!(!r.contains("192.168.0.1"));
    }

    #[test]
    fn redacts_phone() {
        for s in ["+1 555-123-4567", "(555) 123-4567", "555-123-4567"] {
            let r = redact(&format!("call {s} now"));
            assert!(r.contains("[REDACTED:phone]"), "{s} -> {r}");
        }
    }

    #[test]
    fn plain_small_number_not_redacted() {
        let s = "I have 3 apples";
        assert_eq!(redact(s), s);
        assert!(!contains_pii(s));
        assert!(pii_kinds(s).is_empty());
    }

    #[test]
    fn luhn_checks() {
        assert!(luhn_valid("4111111111111111"));
        assert!(!luhn_valid("1234567890123456"));
        assert!(!luhn_valid(""));
        assert!(luhn_valid("4111 1111 1111 1111"));
    }

    #[test]
    fn contains_and_kinds_agree() {
        let s = "mail me at bob@nexus.io or call (555) 123-4567";
        assert!(contains_pii(s));
        let kinds = pii_kinds(s);
        assert!(kinds.contains(&"email".to_string()));
        assert!(kinds.contains(&"phone".to_string()));
        // sorted + deduped
        let mut sorted = kinds.clone();
        sorted.sort();
        assert_eq!(kinds, sorted);
    }

    #[test]
    fn api_key_detected() {
        let r = redact("token AKIAIOSFODNN7EXAMPLE used");
        assert!(r.contains("[REDACTED:api_key]"), "got: {r}");
        let r2 = redact("key sk-abc123DEF456ghi789XYZ done");
        assert!(r2.contains("[REDACTED:api_key]"), "got: {r2}");
    }

    #[test]
    fn no_double_redaction() {
        // The email should win over an api_key-shaped local part.
        let r = redact("user abcdef1234567890ghij@example.com here");
        assert_eq!(r.matches("[REDACTED:").count(), 1, "got: {r}");
        assert!(r.contains("[REDACTED:email]"));
    }
}
