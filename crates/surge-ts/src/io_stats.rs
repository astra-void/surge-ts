//! Process-global I/O counters for the dependency-expansion phases.
//!
//! The package-declaration and import-graph passes read and stat files from
//! deep within recursive resolution helpers that share no timing context, so a
//! handful of relaxed atomics is the least invasive way to attribute their I/O.
//! A single CLI invocation runs one check and exits, so the counters never need
//! resetting between runs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static EXPANSION_FILES_READ: AtomicU64 = AtomicU64::new(0);
static EXPANSION_BYTES_READ: AtomicU64 = AtomicU64::new(0);
static EXPANSION_READ_NANOS: AtomicU64 = AtomicU64::new(0);
static PACKAGE_JSON_READS: AtomicU64 = AtomicU64::new(0);
static FS_EXISTENCE_PROBES: AtomicU64 = AtomicU64::new(0);
static FS_READ_DIR_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn record_expansion_read(bytes: usize, elapsed: Duration) {
    EXPANSION_FILES_READ.fetch_add(1, Ordering::Relaxed);
    EXPANSION_BYTES_READ.fetch_add(bytes as u64, Ordering::Relaxed);
    EXPANSION_READ_NANOS.fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
}

pub fn record_package_json_read() {
    PACKAGE_JSON_READS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_existence_probe() {
    FS_EXISTENCE_PROBES.fetch_add(1, Ordering::Relaxed);
}

pub fn record_read_dir() {
    FS_READ_DIR_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, Default)]
pub struct IoSnapshot {
    pub expansion_files_read: u64,
    pub expansion_bytes_read: u64,
    pub expansion_read_io: Duration,
    pub package_json_reads: u64,
    pub fs_existence_probes: u64,
    pub fs_read_dir_count: u64,
}

pub fn snapshot() -> IoSnapshot {
    IoSnapshot {
        expansion_files_read: EXPANSION_FILES_READ.load(Ordering::Relaxed),
        expansion_bytes_read: EXPANSION_BYTES_READ.load(Ordering::Relaxed),
        expansion_read_io: Duration::from_nanos(EXPANSION_READ_NANOS.load(Ordering::Relaxed)),
        package_json_reads: PACKAGE_JSON_READS.load(Ordering::Relaxed),
        fs_existence_probes: FS_EXISTENCE_PROBES.load(Ordering::Relaxed),
        fs_read_dir_count: FS_READ_DIR_COUNT.load(Ordering::Relaxed),
    }
}
