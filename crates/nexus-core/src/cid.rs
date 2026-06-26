//! Content identifiers: cryptographically secure 512-bit BLAKE3 hashes.
//!
//! Every node in the Nexus graph is identified by a CID computed directly from
//! its canonical byte encoding. Identical content always yields an identical
//! CID (perfect structural deduplication), and any change to content produces a
//! completely different CID (tamper-evidence / native provenance).

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A 512-bit (64-byte) BLAKE3 content identifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cid(pub [u8; 64]);

impl Serialize for Cid {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

struct CidVisitor;

impl<'de> Visitor<'de> for CidVisitor {
    type Value = Cid;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a 128-char hex BLAKE3-512 CID")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Cid, E> {
        Cid::from_hex(v).ok_or_else(|| de::Error::custom("invalid CID hex"))
    }
}

impl<'de> Deserialize<'de> for Cid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Cid, D::Error> {
        d.deserialize_str(CidVisitor)
    }
}

impl Cid {
    /// Compute the CID of an arbitrary byte slice using BLAKE3 in extended
    /// (XOF) mode to produce a full 512-bit digest.
    pub fn of_bytes(bytes: &[u8]) -> Cid {
        let mut hasher = blake3::Hasher::new();
        hasher.update(bytes);
        let mut out = [0u8; 64];
        hasher.finalize_xof().fill(&mut out);
        Cid(out)
    }

    /// Lowercase hex encoding of the full 512-bit digest (128 hex chars).
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(128);
        for b in self.0 {
            s.push(nibble(b >> 4));
            s.push(nibble(b & 0xf));
        }
        s
    }

    /// Short, human/AI-friendly prefix (first 12 hex chars) used in views and
    /// logs. Collisions across a realistic graph are astronomically unlikely.
    pub fn short(&self) -> String {
        self.to_hex()[..12].to_string()
    }

    /// Parse a CID from a full 128-char hex string.
    pub fn from_hex(s: &str) -> Option<Cid> {
        if s.len() != 128 {
            return None;
        }
        let mut out = [0u8; 64];
        let bytes = s.as_bytes();
        for i in 0..64 {
            let hi = unhex(bytes[i * 2])?;
            let lo = unhex(bytes[i * 2 + 1])?;
            out[i] = (hi << 4) | lo;
        }
        Some(Cid(out))
    }
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

fn unhex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cid:{}", self.short())
    }
}

impl fmt::Debug for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cid({})", self.short())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a = Cid::of_bytes(b"nexus");
        let b = Cid::of_bytes(b"nexus");
        assert_eq!(a, b, "identical content must hash equally");
    }

    #[test]
    fn sensitive_to_change() {
        let a = Cid::of_bytes(b"account: 100");
        let b = Cid::of_bytes(b"account: 101");
        assert_ne!(a, b, "any change must produce a different CID");
    }

    #[test]
    fn hex_roundtrip() {
        let c = Cid::of_bytes(b"roundtrip");
        let hex = c.to_hex();
        assert_eq!(hex.len(), 128);
        assert_eq!(Cid::from_hex(&hex), Some(c));
    }

    #[test]
    fn is_512_bit() {
        let c = Cid::of_bytes(b"x");
        assert_eq!(c.0.len(), 64); // 64 bytes = 512 bits
    }
}
