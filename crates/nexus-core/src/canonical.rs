//! Deterministic canonical byte encoding.
//!
//! Content addressing requires that logically identical values always produce
//! identical bytes, regardless of map ordering, float formatting, or platform.
//! This module provides a tiny tagged, length-prefixed writer used to derive
//! every node's CID. It is intentionally independent of `serde` (whose output
//! ordering and float formatting are not guaranteed stable) so the hash is a
//! stable contract.

/// A growable canonical byte sink with primitive write helpers.
#[derive(Default)]
pub struct CanonicalWriter {
    buf: Vec<u8>,
}

impl CanonicalWriter {
    pub fn new() -> Self {
        Self { buf: Vec::with_capacity(64) }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// A single-byte type tag, written before each value to make the encoding
    /// injective across differently-typed values.
    pub fn tag(&mut self, t: u8) -> &mut Self {
        self.buf.push(t);
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn buf_i128(&mut self, v: i128) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// Length-prefixed UTF-8 string.
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.u64(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
        self
    }

    /// Raw length-prefixed bytes.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.u64(b.len() as u64);
        self.buf.extend_from_slice(b);
        self
    }

    /// A length header for a sequence; callers then write each element.
    pub fn seq(&mut self, len: usize) -> &mut Self {
        self.u64(len as u64);
        self
    }
}

/// Trait implemented by everything that participates in content addressing.
pub trait Canonical {
    fn write_canonical(&self, w: &mut CanonicalWriter);

    /// Materialize the full canonical byte vector for this value.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut w = CanonicalWriter::new();
        self.write_canonical(&mut w);
        w.into_bytes()
    }
}
