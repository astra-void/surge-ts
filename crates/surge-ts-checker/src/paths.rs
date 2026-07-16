use std::cell::RefCell;
use std::path::{Path, PathBuf};

use surge_ts_types::fx::FxHashMap;

thread_local! {
    // `std::fs::canonicalize` issues a `realpath()` syscall on every call, and
    // type/module resolution canonicalizes the same handful of paths over and
    // over within a single check. Profiling showed this syscall as the single
    // largest self-time cost. The filesystem and cwd are stable for the
    // duration of a check, so memoizing the result per thread is safe. Worker
    // threads are spawned fresh per run (via `thread::scope`), so their caches
    // never outlive a run; the main thread's cache is cleared at the start of
    // each program check.
    // Keyed by the path's raw bytes: `Path as Hash` normalizes separators
    // component-by-component on every lookup, which showed up in profiles at
    // ~6M lookups/run. Raw-byte identity is stricter than `Path` equality, so
    // at worst two spellings of one path each get their own (identical) entry.
    static CANONICALIZE_CACHE: RefCell<FxHashMap<Box<[u8]>, std::sync::Arc<str>>> =
        RefCell::new(FxHashMap::default());
}

/// Clears the per-thread path canonicalization cache. Called at the start of a
/// program check so a long-lived process (e.g. a test binary running many
/// checks) never observes a stale canonicalization across runs.
pub(crate) fn clear_canonicalize_cache() {
    CANONICALIZE_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn canonicalize_if_exists_string(path: &Path) -> String {
    canonicalize_if_exists_arc(path).as_ref().to_string()
}

/// Shared-handle variant of [`canonicalize_if_exists_string`]: cache hits are
/// a refcount bump instead of a fresh `String`, which matters on the
/// resolution-key path (millions of lookups per run).
pub(crate) fn canonicalize_if_exists_arc(path: &Path) -> std::sync::Arc<str> {
    crate::program::record_string_path_lookup();
    crate::program::record_canonicalize_call();
    CANONICALIZE_CACHE.with(|cache| {
        let key = path.as_os_str().as_encoded_bytes();
        if let Some(cached) = cache.borrow().get(key) {
            crate::program::record_canonicalize_cache_hit();
            return std::sync::Arc::clone(cached);
        }
        let result: std::sync::Arc<str> = std::sync::Arc::from(
            canonicalize_if_exists(path)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        cache
            .borrow_mut()
            .insert(key.into(), std::sync::Arc::clone(&result));
        result
    })
}

pub(crate) fn normalize_path_string(path: &str) -> String {
    crate::program::record_string_path_lookup();
    normalize_path_buf(Path::new(path))
        .to_string_lossy()
        .replace('\\', "/")
}

fn canonicalize_if_exists(path: &Path) -> PathBuf {
    let start = crate::program::counters_enabled().then(std::time::Instant::now);
    let canonical = std::fs::canonicalize(path);
    if let Some(start) = start {
        crate::program::record_canonicalize_syscall(start.elapsed());
    }
    match canonical {
        Ok(canonical) => normalize_path_buf(&canonical),
        Err(_) => normalize_path_buf(path),
    }
}

fn normalize_path_buf(path: &Path) -> PathBuf {
    let path = path.to_string_lossy().replace('\\', "/");
    let is_absolute = path.starts_with('/');
    let mut drive_letter = "";

    let path_to_split = if path.chars().nth(1) == Some(':') {
        drive_letter = &path[0..2];
        &path[2..]
    } else {
        &path
    };

    let mut segments = Vec::new();
    for segment in path_to_split.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            if let Some(last) = segments.last() {
                if last != ".." {
                    segments.pop();
                    continue;
                }
            }

            if !is_absolute && drive_letter.is_empty() {
                segments.push(segment.to_string());
            }

            continue;
        }

        segments.push(segment.to_string());
    }

    let mut result = String::new();
    if !drive_letter.is_empty() {
        result.push_str(drive_letter);
        if path_to_split.starts_with('/') {
            result.push('/');
        }
    } else if is_absolute {
        result.push('/');
    }

    result.push_str(&segments.join("/"));

    if result.is_empty() {
        if is_absolute {
            PathBuf::from("/")
        } else {
            PathBuf::from(".")
        }
    } else {
        PathBuf::from(result)
    }
}
