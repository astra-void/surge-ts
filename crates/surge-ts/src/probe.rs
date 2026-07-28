//! Cached file-existence probing for the loader's resolution passes.
//!
//! Package entrypoint resolution fans each extensionless target out to many
//! candidate paths and re-probes the same candidates from every importer
//! directory that reaches the package. The filesystem is static for the
//! duration of a run, so one `metadata` syscall per unique path answers every
//! later probe. The cache is cleared at the start of `Project::check` so test
//! processes that rebuild fixture trees between checks never observe a stale
//! answer.

use std::cell::RefCell;
use std::path::Path;
use surge_ts_types::fx::FxHashMap;

thread_local! {
    static PROBE_CACHE: RefCell<FxHashMap<Box<[u8]>, bool>> = RefCell::new(FxHashMap::default());
}

pub(crate) fn clear_probe_cache() {
    PROBE_CACHE.with(|cache| cache.borrow_mut().clear());
}

/// Whether `path` names an existing regular file, memoized per thread.
pub(crate) fn is_existing_file(path: &Path) -> bool {
    PROBE_CACHE.with(|cache| {
        let key = path.as_os_str().as_encoded_bytes();
        if let Some(&hit) = cache.borrow().get(key) {
            return hit;
        }
        let probe_start = std::time::Instant::now();
        let is_file = std::fs::metadata(path)
            .map(|metadata| metadata.is_file())
            .unwrap_or(false);
        crate::io_stats::record_existence_probe(probe_start.elapsed());
        crate::io_stats::record_probe_parent(path);
        cache.borrow_mut().insert(key.into(), is_file);
        is_file
    })
}
