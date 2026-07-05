//! A tiny, dependency-free FxHash-style hasher for interpreter-internal maps.
//!
//! Variable names and function names are short ASCII strings hashed on every
//! scope read/write, so the default SipHash (DoS-resistant, but slow) costs
//! real interpreter throughput. Interpreter scopes are not attacker-controlled
//! hash tables in the DoS sense — the program author already controls the
//! whole program — so a fast non-cryptographic hash is the right trade.

use std::hash::{BuildHasherDefault, Hasher};

/// Multiplicative hash constant (same one rustc's FxHash uses).
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().unwrap()));
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut word = 0u64;
            for (i, &b) in rem.iter().enumerate() {
                word |= (b as u64) << (8 * i);
            }
            // Fold in the remainder length so "a" and "a\0" differ.
            self.add(word ^ ((rem.len() as u64) << 56));
        }
    }

    #[inline]
    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }

    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

/// A `HashMap` keyed by short interpreter strings (variable/function names).
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_keys_hash_differently() {
        let mut m: FxHashMap<String, i32> = FxHashMap::default();
        for i in 0..1000 {
            m.insert(format!("var_{i}"), i);
        }
        for i in 0..1000 {
            assert_eq!(m[&format!("var_{i}")], i);
        }
    }

    #[test]
    fn prefix_keys_do_not_collide() {
        let hash = |s: &str| {
            let mut h = FxHasher::default();
            h.write(s.as_bytes());
            h.finish()
        };
        assert_ne!(hash("a"), hash("aa"));
        assert_ne!(hash("counter"), hash("counters"));
    }
}
