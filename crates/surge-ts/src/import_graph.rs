use std::fs;
use std::path::{Path, PathBuf};

use surge_ts_checker::SourceFileInput;
use surge_ts_checker::lowlevel::resolution_candidates::{
    mapped_target_candidates, relative_import_candidates,
};
use surge_ts_config::PathMapping;
use surge_ts_config::{
    canonicalize_if_exists, canonicalize_if_exists_string, normalize_path_string,
    select_path_mapping_targets,
};

use crate::specifier_scan::ModuleSpecifierScanner;

/// Import-graph BFS state that survives across loader fixpoint iterations, so
/// each source is scanned exactly once no matter how many times the loader
/// loop re-enters the expansion.
#[derive(Default)]
pub struct ImportGraphState {
    known_files: surge_ts_types::fx::FxHashSet<String>,
    probe_cache: surge_ts_types::fx::FxHashMap<String, bool>,
    next_source_index: usize,
    synced_inputs: usize,
}

pub fn expand_project_inputs(
    state: &mut ImportGraphState,
    scanner: &mut ModuleSpecifierScanner,
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    base_url: Option<&Path>,
    paths: &[PathMapping],
) -> usize {
    // Other scanners (package declarations, reference directives) may have
    // appended inputs since the previous call; fold them into the known set so
    // their files are not re-added under a second path spelling.
    for input in inputs[state.synced_inputs..].iter() {
        state
            .known_files
            .insert(canonicalize_if_exists_string(Path::new(&input.file_name)));
    }

    let mut added = 0usize;

    // Frontier-at-a-time BFS. Resolution stays serial (it mutates the probe
    // cache and the known-file set), but a wave's file reads are independent,
    // so they run on a pool and the discovered sources are appended in the
    // exact order the file-at-a-time loop would have appended them.
    while state.next_source_index < sources.len() {
        let wave_end = sources.len();
        scanner.prefetch(sources, state.next_source_index);

        let mut discovered: Vec<PathBuf> = Vec::new();
        while state.next_source_index < wave_end {
            let index = state.next_source_index;
            state.next_source_index += 1;

            let (file_path, file_name, source_text) = {
                let (file_path, file_name, source_text) = &sources[index];
                (file_path.clone(), file_name.clone(), source_text.clone())
            };

            for module_specifier in scanner.specifiers(index, &file_name, &source_text).iter() {
                let candidate = if is_relative_specifier(module_specifier) {
                    resolve_relative_candidate(&file_path, module_specifier, &mut state.probe_cache)
                } else {
                    resolve_paths_alias_candidate(
                        module_specifier,
                        paths,
                        base_url,
                        root_dir,
                        &mut state.probe_cache,
                    )
                };

                let Some(candidate) = candidate else {
                    continue;
                };

                if is_dependency_javascript_source_file(&candidate)
                    || !is_loadable_graph_file(&candidate)
                {
                    continue;
                }

                let canonical = canonicalize_if_exists(&candidate);
                let normalized = canonicalize_if_exists_string(&canonical);
                record_graph_edge(&file_name, &normalized);
                if !state.known_files.insert(normalized) {
                    continue;
                }

                discovered.push(canonical);
            }
        }

        for (canonical, source_text) in read_discovered_sources(discovered) {
            let Some(source_text) = source_text else {
                continue;
            };
            let file_name = canonical.to_string_lossy().into_owned();
            inputs.push(SourceFileInput {
                file_name: file_name.clone(),
                source_text: source_text.clone(),
            });
            sources.push((canonical, file_name, source_text));
            added += 1;
        }
    }

    state.synced_inputs = inputs.len();
    added
}

/// Read a discovery wave's files, preserving input order. Unreadable files come
/// back as `None` so the caller skips them exactly as the serial read did.
fn read_discovered_sources(paths: Vec<PathBuf>) -> Vec<(PathBuf, Option<String>)> {
    let read_one = |path: &Path| {
        let read_start = std::time::Instant::now();
        let text = fs::read_to_string(path).ok();
        if let Some(text) = &text {
            crate::io_stats::record_expansion_read(text.len(), read_start.elapsed());
        }
        text
    };

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(paths.len());
    if paths.len() < 8 || workers <= 1 {
        return paths
            .into_iter()
            .map(|path| {
                let text = read_one(&path);
                (path, text)
            })
            .collect();
    }

    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut results: Vec<(usize, Option<String>)> = std::thread::scope(|scope| {
        let paths = &paths;
        let next = &next;
        let read_one = &read_one;
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            handles.push(scope.spawn(move || {
                let mut out = Vec::new();
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= paths.len() {
                        break;
                    }
                    out.push((index, read_one(&paths[index])));
                }
                out
            }));
        }
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("expansion read worker panicked"))
            .collect()
    });
    results.sort_unstable_by_key(|(index, _)| *index);
    paths
        .into_iter()
        .zip(results)
        .map(|(path, (_, text))| (path, text))
        .collect()
}

/// Opt-in module import-edge dump (`SURGE_MODULE_GRAPH_DUMP=<path>`): appends one
/// tab-separated `importer\timportee` line per resolved relative / `paths` edge,
/// including back-edges to already-discovered files, so the full cyclic import
/// graph (not just the discovery tree) can be reconstructed offline for SCC /
/// critical-path analysis. Off by default and zero-cost when unset.
fn record_graph_edge(importer: &str, importee: &str) {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    static SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    let sink = SINK.get_or_init(|| {
        let path = std::env::var_os("SURGE_MODULE_GRAPH_DUMP")?;
        std::fs::File::create(path).ok().map(Mutex::new)
    });
    if let Some(sink) = sink
        && let Ok(mut file) = sink.lock()
    {
        let _ = writeln!(file, "{importer}\t{importee}");
    }
}

fn resolve_relative_candidate(
    importer_file: &Path,
    specifier: &str,
    probe_cache: &mut surge_ts_types::fx::FxHashMap<String, bool>,
) -> Option<PathBuf> {
    let importer_dir = importer_file.parent().unwrap_or_else(|| Path::new(""));
    let normalized_specifier = normalize_path_string(specifier);
    let joined = normalize_path_string(&importer_dir.join(&normalized_specifier).to_string_lossy());

    let candidate_paths = relative_import_candidates(&joined, &normalized_specifier)?;

    for candidate in candidate_paths {
        let candidate = PathBuf::from(candidate);
        if !candidate_is_existing_file(&candidate, probe_cache) {
            continue;
        }

        if is_dependency_javascript_source_file(&candidate) || !is_loadable_graph_file(&candidate) {
            continue;
        }

        return Some(candidate);
    }

    None
}

fn resolve_paths_alias_candidate(
    specifier: &str,
    paths: &[PathMapping],
    base_url: Option<&Path>,
    root_dir: &Path,
    probe_cache: &mut surge_ts_types::fx::FxHashMap<String, bool>,
) -> Option<PathBuf> {
    // `paths` substitutions and the bare-import fallback resolve against
    // `baseUrl` when set, else the config directory (tsc ≥4.4 allows `paths`
    // without `baseUrl`).
    let mapping_base = base_url.unwrap_or(root_dir);

    if let Some(targets) = select_path_mapping_targets(specifier, paths) {
        for target in targets {
            let joined = normalize_path_string(&mapping_base.join(&target).to_string_lossy());
            if let Some(candidate) = probe_loadable_candidates(&joined, probe_cache) {
                return Some(candidate);
            }
        }
        return None;
    }

    // No pattern matched: tsc falls back to resolving the bare specifier
    // directly against `baseUrl`.
    if let Some(base_url) = base_url {
        let joined = normalize_path_string(&base_url.join(specifier).to_string_lossy());
        return probe_loadable_candidates(&joined, probe_cache);
    }

    None
}

fn probe_loadable_candidates(
    target: &str,
    probe_cache: &mut surge_ts_types::fx::FxHashMap<String, bool>,
) -> Option<PathBuf> {
    for candidate in mapped_target_candidates(target) {
        let candidate = PathBuf::from(candidate);
        if !candidate_is_existing_file(&candidate, probe_cache) {
            continue;
        }

        if is_dependency_javascript_source_file(&candidate) || !is_loadable_graph_file(&candidate) {
            continue;
        }

        return Some(candidate);
    }

    None
}

// Most extensionless specifiers fan out to ~15 candidate paths and modules are
// imported from many files, so hits are memoized by path; misses go through
// the shared probe layer, which upgrades hot directories to a one-shot
// `read_dir` listing.
fn candidate_is_existing_file(
    candidate: &Path,
    cache: &mut surge_ts_types::fx::FxHashMap<String, bool>,
) -> bool {
    let key = candidate.to_string_lossy();
    if let Some(&hit) = cache.get(key.as_ref()) {
        return hit;
    }
    let is_file = crate::probe::is_existing_file(candidate);
    cache.insert(key.into_owned(), is_file);
    is_file
}

// Byte-level ASCII-case-insensitive matching: these checks run per resolved
// candidate, and the previous `to_string_lossy().to_ascii_lowercase()` paid
// two allocations each. Lossy conversion only replaces invalid UTF-8 with
// U+FFFD (non-ASCII), so ASCII needles match the lossy string iff they match
// the raw bytes.
fn path_ends_with_ignore_ascii_case(path: &Path, suffix: &str) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    bytes.len() >= suffix.len()
        && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

fn path_contains_ignore_ascii_case(path: &Path, needle: &str) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn is_loadable_graph_file(path: &Path) -> bool {
    // TypeScript sources are loadable even under `node_modules`: tsc applies the
    // same extension priority (`.ts` before `.d.ts`) to relative imports inside
    // dependencies, so a package that ships sources (or is resolved through a
    // source `exports` condition) gets its source graph checked, not its
    // declarations.
    path_ends_with_ignore_ascii_case(path, ".ts")
        || path_ends_with_ignore_ascii_case(path, ".tsx")
        || path_ends_with_ignore_ascii_case(path, ".mts")
        || path_ends_with_ignore_ascii_case(path, ".cts")
}

fn is_dependency_javascript_source_file(path: &Path) -> bool {
    let is_node_modules = path_contains_ignore_ascii_case(path, "/node_modules/")
        || path_contains_ignore_ascii_case(path, "\\node_modules\\");
    let is_javascript_source = path_ends_with_ignore_ascii_case(path, ".js")
        || path_ends_with_ignore_ascii_case(path, ".jsx")
        || path_ends_with_ignore_ascii_case(path, ".mjs")
        || path_ends_with_ignore_ascii_case(path, ".cjs");

    is_node_modules && is_javascript_source
}

fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with(".\\")
        || specifier.starts_with("..\\")
}
