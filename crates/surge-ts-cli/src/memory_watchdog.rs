//! Opt-in memory guard: `SURGE_MAX_FOOTPRINT_MB=<n>` terminates the process
//! once its physical footprint crosses `n` MiB.
//!
//! macOS has no per-process OOM killer; a runaway type expansion swaps and
//! compresses until the whole machine locks up (55 GB on the 2026-09-10
//! tanstack intersection cycle, 60 GB on 2026-09-12 took the host down). The
//! guard exists so a harness can bound that blast radius from inside the
//! process, where `RLIMIT_AS`/`RLIMIT_RSS` are not enforced.
//!
//! The sample is `phys_footprint` where the platform reports it and the
//! resident set otherwise. RSS undercounts under memory pressure — exactly when
//! the guard matters — so the footprint is preferred whenever available.
//!
//! Never active without the env var; touches nothing on the checking path.

use std::time::Duration;

use crate::probe;

pub(crate) const ENV_VAR: &str = "SURGE_MAX_FOOTPRINT_MB";

/// The conventional "killed for memory" status (128 + SIGKILL), so harnesses
/// that already classify OOM kills by exit code classify this the same way.
/// A real SIGKILL reaches the parent as a signal, not a status, so the two stay
/// distinguishable.
pub(crate) const EXIT_CODE: i32 = 137;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Read the limit from the environment and, if set, sample once right away and
/// then start a background sampler. Sampling before returning makes a limit
/// the process already exceeds fire deterministically, before any checking
/// thread is spawned.
pub(crate) fn start_from_env() -> Result<(), String> {
    let Some(raw) = std::env::var_os(ENV_VAR) else {
        return Ok(());
    };
    let limit_mib = parse_limit_mib(&raw.to_string_lossy())
        .map_err(|reason| format!("{ENV_VAR}={raw:?}: {reason}"))?;
    let limit_bytes = limit_mib.saturating_mul(1024 * 1024);
    enforce(limit_bytes);
    std::thread::Builder::new()
        .name("surge-memory-watchdog".into())
        .spawn(move || {
            loop {
                std::thread::sleep(POLL_INTERVAL);
                enforce(limit_bytes);
            }
        })
        .map_err(|error| format!("could not spawn the memory watchdog thread: {error}"))?;
    Ok(())
}

pub(crate) fn parse_limit_mib(raw: &str) -> Result<u64, String> {
    let value: u64 = raw
        .trim()
        .parse()
        .map_err(|_| "expected a whole number of MiB".to_string())?;
    if value == 0 {
        return Err("must be greater than 0".to_string());
    }
    Ok(value)
}

fn sample_bytes() -> Option<u64> {
    probe::current_footprint_bytes().or_else(probe::current_rss_bytes)
}

fn enforce(limit_bytes: u64) {
    let Some(current) = sample_bytes() else {
        return;
    };
    if current <= limit_bytes {
        return;
    }
    let stage = surge_ts_checker::lowlevel::last_rss_stage_label().unwrap_or("before check");
    eprintln!(
        "surge: memory limit exceeded: {} > {} ({ENV_VAR}), last stage: {stage}; exiting {EXIT_CODE}",
        format_mib(current),
        format_mib(limit_bytes),
    );
    std::process::exit(EXIT_CODE);
}

fn format_mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::parse_limit_mib;

    #[test]
    fn limit_parses_whole_mib_only() {
        assert_eq!(parse_limit_mib("8192"), Ok(8192));
        assert_eq!(parse_limit_mib(" 16 "), Ok(16));
        assert!(parse_limit_mib("0").is_err());
        assert!(parse_limit_mib("8g").is_err());
        assert!(parse_limit_mib("").is_err());
        assert!(parse_limit_mib("-1").is_err());
    }
}
