//! Instrumentation: program-wide performance counters and phase timings.
//!
//! Pure diagnostics, gated behind `--timings` / `COUNTERS_ENABLED`. Split out of
//! `program.rs` so the checking pipeline reads without the counter plumbing
//! interleaved. Re-exported from `program` for `crate::program::record_*` callers.

mod counters;
mod rss;
mod timings;

pub(crate) use counters::*;
pub(crate) use timings::*;

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
    eprintln!(
        "RSS loader stage: {label} rss={} peak={}",
        timings::format_bytes_opt(current),
        timings::format_bytes_opt(peak),
    );
    if rss_json_enabled() {
        eprintln!(
            "{{\"rssLoaderStage\":\"{label}\",\"rssBytes\":{},\"peakRssBytes\":{}}}",
            json_u64_opt(current),
            json_u64_opt(peak),
        );
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
