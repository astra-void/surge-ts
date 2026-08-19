//! Instrumentation: program-wide performance counters and phase timings.
//!
//! Pure diagnostics, gated behind `--timings` / `COUNTERS_ENABLED`. Split out of
//! `program.rs` so the checking pipeline reads without the counter plumbing
//! interleaved. Re-exported from `program` for `crate::program::record_*` callers.

mod counters;
mod dts_expansion;
mod retention_census;
pub(crate) mod rss;
mod timings;
mod type_graph_census;

pub(crate) use counters::*;
pub(crate) use dts_expansion::*;
pub(crate) use retention_census::*;
pub(crate) use rss::{current_footprint_bytes, peak_footprint_bytes};
pub(crate) use timings::*;
pub(crate) use type_graph_census::*;

/// RSS probe for loader-side phases (config load, source read, import-graph
/// expansion) that run before `check_program` and therefore have no timings
/// carrier. Gated on the same `SURGE_RSS`/`SURGE_TIMINGS` opt-in as the stage
/// table and emitted immediately to stderr; `SURGE_RSS_JSON` additionally emits
/// one machine-readable JSON line per probe.
pub fn record_loader_rss_stage(label: &str) {
    if !rss_stages_enabled() {
        return;
    }
    let current = rss::current_rss_bytes();
    let peak = rss::peak_rss_bytes();
    let footprint = rss::current_footprint_bytes();
    let peak_footprint = rss::peak_footprint_bytes();
    eprintln!(
        "RSS loader stage: {label} rss={} peak={} fp={} fp_peak={}",
        timings::format_bytes_opt(current),
        timings::format_bytes_opt(peak),
        timings::format_bytes_opt(footprint),
        timings::format_bytes_opt(peak_footprint),
    );
    if rss_json_enabled() {
        eprintln!(
            "{{\"rssLoaderStage\":\"{label}\",\"rssBytes\":{},\"peakRssBytes\":{},\
             \"footprintBytes\":{},\"peakFootprintBytes\":{}}}",
            json_u64_opt(current),
            json_u64_opt(peak),
            json_u64_opt(footprint),
            json_u64_opt(peak_footprint),
        );
    }
}

/// Returns freed allocator memory to the OS. The module-analysis pipeline drops
/// whole superseded generations (preliminary analyses, round-1/2 binding tables)
/// whose pages otherwise stay dirty in malloc freelists, get compressed under
/// pressure, and keep counting against the process physical footprint through
/// the check-phase peak. Called at the few lifecycle boundaries where a large
/// generation has just been dropped.
pub(crate) fn release_free_memory() {
    #[cfg(feature = "mimalloc")]
    unsafe {
        libmimalloc_sys::mi_collect(true);
    }
    #[cfg(all(not(feature = "mimalloc"), target_os = "macos"))]
    {
        unsafe extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
        }
        unsafe {
            malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
}

pub(crate) fn rss_stages_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("SURGE_RSS").is_some() || std::env::var_os("SURGE_TIMINGS").is_some()
    })
}

pub(crate) fn rss_json_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_RSS_JSON").is_some())
}

pub(crate) fn json_u64_opt(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}
