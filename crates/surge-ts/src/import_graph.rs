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

    scanner.prefetch(sources, state.next_source_index);
    while state.next_source_index < sources.len() {
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

            let read_start = std::time::Instant::now();
            let Ok(source_text) = fs::read_to_string(&canonical) else {
                continue;
            };
            crate::io_stats::record_expansion_read(source_text.len(), read_start.elapsed());

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

// Each probe previously issued two stat syscalls (`exists()` then `is_file()`).
// A single `metadata` call answers both, and most extensionless specifiers fan
// out to ~15 candidate paths, so caching by path collapses repeated probes for
// modules imported from many files.
fn candidate_is_existing_file(
    candidate: &Path,
    cache: &mut surge_ts_types::fx::FxHashMap<String, bool>,
) -> bool {
    let key = candidate.to_string_lossy();
    if let Some(&hit) = cache.get(key.as_ref()) {
        return hit;
    }
    let probe_start = std::time::Instant::now();
    let is_file = fs::metadata(candidate)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    crate::io_stats::record_existence_probe(probe_start.elapsed());
    cache.insert(key.into_owned(), is_file);
    is_file
}

fn is_loadable_graph_file(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    // TypeScript sources are loadable even under `node_modules`: tsc applies the
    // same extension priority (`.ts` before `.d.ts`) to relative imports inside
    // dependencies, so a package that ships sources (or is resolved through a
    // source `exports` condition) gets its source graph checked, not its
    // declarations.
    lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".mts")
        || lower.ends_with(".cts")
}

fn is_dependency_javascript_source_file(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    let is_node_modules = lower.contains("/node_modules/") || lower.contains("\\node_modules\\");
    let is_javascript_source = lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs");

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
