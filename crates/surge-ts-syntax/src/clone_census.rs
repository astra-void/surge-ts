//! Opt-in census of `ParsedType` clones, keyed by top-level variant.
//!
//! Enabled by `SURGE_ALLOCATION_CENSUS=1` (read once). When disabled the
//! per-clone cost is a single relaxed load and predicted branch. The census
//! distinguishes composite variants (whose clones allocate recursively) from
//! primitive ones (enum copies), so allocation-volume work can verify both the
//! clone count and — after representation changes — that formerly-deep clones
//! became shallow.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub const PARSED_TYPE_VARIANTS: [&str; 12] = [
    "primitive",
    "string_literal",
    "number_literal",
    "object",
    "array_or_keyof",
    "tuple_union_intersection",
    "function",
    "named",
    "typeof",
    "indexed_access",
    "mapped_or_conditional",
    "template_or_infer",
];

static CLONES: [AtomicU64; 12] = [const { AtomicU64::new(0) }; 12];

pub(crate) fn census_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_ALLOCATION_CENSUS").is_some())
}

#[inline]
pub(crate) fn record_parsed_type_clone(variant: usize) {
    if census_enabled() {
        CLONES[variant].fetch_add(1, Ordering::Relaxed);
    }
}

/// `(variant name, clone count)` pairs since process start (or last reset).
pub fn parsed_type_clone_census() -> Vec<(&'static str, u64)> {
    PARSED_TYPE_VARIANTS
        .iter()
        .zip(CLONES.iter())
        .map(|(name, count)| (*name, count.load(Ordering::Relaxed)))
        .collect()
}

pub fn reset_parsed_type_clone_census() {
    for counter in CLONES.iter() {
        counter.store(0, Ordering::Relaxed);
    }
}
