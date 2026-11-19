//! A fast non-cryptographic hasher for maps keyed by the game's own ids
//! (FEng object / name hashes, texture hashes, indices), in place of std's
//! SipHash (docs/notes/performance/main-thread.md). It is the Fx hash (Firefox,
//! rustc: `rustc-hash` 1.x), kept here so the crate needs no new
//! dependency.
//!
//! Only for maps whose iteration order cannot reach the output: std's
//! `RandomState` already visits a map in a different order every run, so
//! a map that the game's output does not depend on today (frames, sound and
//! simulation identical run to run, the parity gates) does not depend on it with
//! this hasher either; this one's order is the same every run.

use std::hash::{BuildHasherDefault, Hasher};

/// `HashMap` with [`FxHasher`].
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;
/// `HashSet` with [`FxHasher`].
pub type FxHashSet<K> = std::collections::HashSet<K, FxBuild>;
/// The builder (stateless: every map hashes alike).
pub type FxBuild = BuildHasherDefault<FxHasher>;

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// The Fx hash: per word, rotate, xor and multiply.
#[derive(Clone, Copy, Default)]
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
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut w = [0u8; 8];
            w[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(w));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_behave_as_std_maps() {
        let mut m: FxHashMap<u32, u32> = FxHashMap::default();
        let mut s: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let mut x = 0x1234_5678u32;
        for i in 0..10_000u32 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            m.insert(x, i);
            s.insert(x, i);
            if i % 3 == 0 {
                m.remove(&(x ^ 1));
                s.remove(&(x ^ 1));
            }
        }
        assert_eq!(m.len(), s.len());
        assert!(s.iter().all(|(k, v)| m.get(k) == Some(v)));
        // Sequential indices spread over the buckets (the multiply is odd).
        let seq: FxHashSet<usize> = (0..4096).collect();
        assert!((0..4096).all(|i| seq.contains(&i)));
    }

    #[test]
    fn hashes_are_fixed() {
        use std::hash::BuildHasher;
        let b = FxBuild::default();
        assert_eq!(b.hash_one(0u32), 0);
        assert_eq!(b.hash_one(1u32), SEED);
        assert_eq!(b.hash_one(1u32), FxBuild::default().hash_one(1u32));
    }
}
