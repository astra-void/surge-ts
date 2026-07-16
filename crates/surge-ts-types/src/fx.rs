//! Fast non-cryptographic hashing for interner and cache maps.
//!
//! The default `std` hasher (SipHash-1-3) is DoS-resistant but measurably slow
//! on the checker's hot lookup paths (canonical-store fingerprints, resolution
//! caches, path caches — millions of lookups per run). These maps are never
//! exposed to attacker-controlled keys in a way where collision-flooding is a
//! concern (keys are source file paths, declaration names, and 64-bit
//! fingerprints), so a fast hasher is appropriate. None of these maps leak
//! iteration order into diagnostics.
//!
//! `FxHasher` is the FxHash algorithm used by rustc (a multiply-xor hash over
//! machine words). `PrehashedU64` is an identity hasher for maps whose keys are
//! already high-entropy 64-bit fingerprints — re-hashing those through any
//! algorithm is pure overhead.

use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// The rustc FxHash algorithm (word-at-a-time multiply-xor).
#[derive(Default, Clone)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        while bytes.len() >= 8 {
            let mut buffer = [0u8; 8];
            buffer.copy_from_slice(&bytes[..8]);
            self.add_to_hash(u64::from_le_bytes(buffer));
            bytes = &bytes[8..];
        }
        if bytes.len() >= 4 {
            let mut buffer = [0u8; 4];
            buffer.copy_from_slice(&bytes[..4]);
            self.add_to_hash(u64::from(u32::from_le_bytes(buffer)));
            bytes = &bytes[4..];
        }
        if bytes.len() >= 2 {
            let mut buffer = [0u8; 2];
            buffer.copy_from_slice(&bytes[..2]);
            self.add_to_hash(u64::from(u16::from_le_bytes(buffer)));
            bytes = &bytes[2..];
        }
        if let Some(&byte) = bytes.first() {
            self.add_to_hash(u64::from(byte));
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add_to_hash(u64::from(value));
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.add_to_hash(u64::from(value));
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add_to_hash(u64::from(value));
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add_to_hash(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add_to_hash(value as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// Identity hasher for keys that are already high-entropy 64-bit fingerprints.
/// Panics (via `debug_assert`) if used with multi-write keys.
#[derive(Default, Clone)]
pub struct PrehashedU64Hasher {
    hash: u64,
}

impl Hasher for PrehashedU64Hasher {
    #[inline]
    fn write(&mut self, _bytes: &[u8]) {
        debug_assert!(false, "PrehashedU64Hasher only supports u64 keys");
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.hash = value;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FxBuildHasher = BuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;
pub type FxHashSet<K> = std::collections::HashSet<K, FxBuildHasher>;
pub type PrehashedU64BuildHasher = BuildHasherDefault<PrehashedU64Hasher>;
pub type PrehashedU64Map<V> = std::collections::HashMap<u64, V, PrehashedU64BuildHasher>;
