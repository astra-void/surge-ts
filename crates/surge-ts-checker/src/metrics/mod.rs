//! Instrumentation: program-wide performance counters and phase timings.
//!
//! Pure diagnostics, gated behind `--timings` / `COUNTERS_ENABLED`. Split out of
//! `program.rs` so the checking pipeline reads without the counter plumbing
//! interleaved. Re-exported from `program` for `crate::program::record_*` callers.

mod counters;
mod timings;

pub(crate) use counters::*;
pub(crate) use timings::*;
