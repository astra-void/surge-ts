use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde_json::Value;
use walkdir::WalkDir;

use crate::diagnostics::{ConfigDiagnostic, ConfigDiagnosticCode};
use crate::model::NormalizedCompilerOptions;
use crate::paths::{canonicalize_if_exists, resolve_path};

pub(crate) fn resolve_source_files(
    root_dir: &Path,
    files: Option<&Vec<Value>>,
    include: Option<&Vec<Value>>,
    exclude: Option<&Vec<Value>>,
    compiler_options: &NormalizedCompilerOptions,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<PathBuf> {
    if let Some(files) = files {
        return resolve_explicit_files(root_dir, files, diagnostics);
    }

    let include_patterns = match include {
        Some(entries) => parse_pattern_list(
            entries,
            root_dir,
            ConfigDiagnosticCode::InvalidIncludeEntry,
            diagnostics,
        ),
        None => vec!["**/*".to_string()],
    };
    let include_roots = collect_literal_include_roots(root_dir, &include_patterns);
    let mut exclude_patterns = vec![
        "**/node_modules".to_string(),
        "**/node_modules/**".to_string(),
        "**/bower_components".to_string(),
        "**/bower_components/**".to_string(),
        "**/jspm_packages".to_string(),
        "**/jspm_packages/**".to_string(),
    ];
    if let Some(entries) = exclude {
        exclude_patterns.extend(parse_pattern_list(
            entries,
            root_dir,
            ConfigDiagnosticCode::InvalidExcludeEntry,
            diagnostics,
        ));
    }

    let (include_set, include_depths) = build_include_globset(&include_patterns, diagnostics, root_dir);
    let exclude_set = build_globset(&exclude_patterns, diagnostics, root_dir);

    let mut files = Vec::new();
    let mut matched = Vec::new();
    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !should_prune(entry.path(), root_dir, exclude_set.as_ref())
                && !is_unreachable_dot_directory(entry, root_dir, &include_depths, &include_roots)
        })
    {
        let Ok(entry) = entry else {
            continue;
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(relative) = path.strip_prefix(root_dir).ok() else {
            continue;
        };

        if let Some(set) = include_set.as_ref() {
            set.matches_into(relative, &mut matched);
            let by_pattern = matched
                .iter()
                .any(|&index| wildcard_segments_are_visible(relative, include_depths[index]));
            if !by_pattern && !is_under_any_include_root(relative, &include_roots) {
                continue;
            }
        }

        if is_supported_source_file(path, compiler_options.allow_js) {
            files.push(canonicalize_if_exists(path));
        }
    }

    files.sort();
    files.dedup();
    files
}

fn collect_literal_include_roots(root_dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
    patterns
        .iter()
        .filter_map(|pattern| literal_include_root(root_dir, pattern))
        .collect()
}

fn literal_include_root(root_dir: &Path, pattern: &str) -> Option<PathBuf> {
    if contains_glob_metacharacters(pattern) {
        return None;
    }

    // `isImplicitGlob`: tsc only expands a bare entry to `<entry>/**/*` when its
    // last component has no `.`, `*` or `?`. `include: ["src/.generated"]` is
    // therefore a *file* spec that matches nothing, not a directory root.
    let last_component = Path::new(pattern)
        .components()
        .next_back()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    if last_component.contains('.') {
        return None;
    }

    let candidate = resolve_path(root_dir, pattern);
    if candidate.exists() && candidate.is_dir() {
        return Some(
            candidate
                .strip_prefix(root_dir)
                .map(Path::to_path_buf)
                .unwrap_or(candidate),
        );
    }

    None
}

fn contains_glob_metacharacters(pattern: &str) -> bool {
    pattern
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

/// How many leading components an include pattern spells out literally, i.e. the
/// index of its first component containing a wildcard.
fn literal_component_depth(pattern: &str) -> usize {
    Path::new(pattern)
        .components()
        .position(|component| {
            contains_glob_metacharacters(&component.as_os_str().to_string_lossy())
        })
        .unwrap_or_else(|| Path::new(pattern).components().count())
}

/// tsc's include matcher never lets a wildcard match a path segment starting with
/// `.`: its recursive fragment is `[^/.][^/]*` and a leading-`*` component is
/// compiled as `([^./][^/]*)?`. Segments the pattern spells out literally are
/// exempt, which is why `include: ["src/.generated/**/*"]` still works while
/// `include: ["src"]` skips `src/.generated` entirely.
fn wildcard_segments_are_visible(relative: &Path, literal_depth: usize) -> bool {
    relative
        .components()
        .skip(literal_depth)
        .all(|component| !component.as_os_str().to_string_lossy().starts_with('.'))
}

fn is_under_any_include_root(relative: &Path, include_roots: &[PathBuf]) -> bool {
    include_roots.iter().any(|root| {
        relative.starts_with(root)
            && wildcard_segments_are_visible(relative, root.components().count())
    })
}

/// Prune a dot-directory no include pattern can reach. Walking it would only
/// produce files the wildcard rule rejects, so this is a pure saving; a pattern
/// that names the directory literally keeps it alive.
fn is_unreachable_dot_directory(
    entry: &walkdir::DirEntry,
    root_dir: &Path,
    include_depths: &[usize],
    include_roots: &[PathBuf],
) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let Ok(relative) = entry.path().strip_prefix(root_dir) else {
        return false;
    };
    let depth = relative.components().count();
    if depth == 0 {
        return false;
    }
    if !entry.file_name().to_string_lossy().starts_with('.') {
        return false;
    }

    // The directory sits at index `depth - 1`; any pattern whose literal prefix
    // reaches at least that far may still spell it out.
    let literal_reach = depth - 1;
    !include_depths.iter().any(|&d| d > literal_reach)
        && !include_roots
            .iter()
            .any(|root| root.components().count() > literal_reach)
}

fn resolve_explicit_files(
    root_dir: &Path,
    files: &[Value],
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for file in files {
        let Some(raw) = file.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidFilesEntry,
                message: "files entries must be strings".to_string(),
                file_name: root_dir.to_path_buf(),
            });
            continue;
        };

        let candidate = resolve_path(root_dir, raw);
        if !candidate.exists() || !candidate.is_file() {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidFilesEntry,
                message: format!("file `{raw}` does not exist"),
                file_name: root_dir.to_path_buf(),
            });
            continue;
        }

        if !is_supported_source_file(&candidate, false) {
            continue;
        }

        let candidate = canonicalize_if_exists(&candidate);
        if seen.insert(candidate.clone()) {
            results.push(candidate);
        }
    }

    results
}

fn parse_pattern_list(
    values: &[Value],
    file_name: &Path,
    code: ConfigDiagnosticCode,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<String> {
    let mut patterns = Vec::new();
    for value in values {
        let Some(pattern) = value.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code,
                message: "pattern entries must be strings".to_string(),
                file_name: file_name.to_path_buf(),
            });
            continue;
        };
        patterns.push(pattern.to_string());
    }
    patterns
}

/// Like [`build_globset`], but also returns each accepted pattern's literal
/// component depth, positionally aligned with the globset's match indices.
fn build_include_globset(
    patterns: &[String],
    diagnostics: &mut Vec<ConfigDiagnostic>,
    file_name: &Path,
) -> (Option<GlobSet>, Vec<usize>) {
    let mut accepted = Vec::new();
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
                accepted.push(literal_component_depth(pattern));
            }
            Err(error) => diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("invalid glob `{pattern}`: {error}"),
                file_name: file_name.to_path_buf(),
            }),
        }
    }

    match builder.build() {
        Ok(set) => (Some(set), accepted),
        Err(error) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("failed to build globset: {error}"),
                file_name: file_name.to_path_buf(),
            });
            (None, Vec::new())
        }
    }
}

fn build_globset(
    patterns: &[String],
    diagnostics: &mut Vec<ConfigDiagnostic>,
    file_name: &Path,
) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(error) => diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("invalid glob `{pattern}`: {error}"),
                file_name: file_name.to_path_buf(),
            }),
        }
    }

    match builder.build() {
        Ok(set) => Some(set),
        Err(error) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("failed to build globset: {error}"),
                file_name: file_name.to_path_buf(),
            });
            None
        }
    }
}

fn should_prune(path: &Path, root_dir: &Path, exclude_set: Option<&GlobSet>) -> bool {
    if path == root_dir {
        return false;
    }

    let Some(relative) = path.strip_prefix(root_dir).ok() else {
        return false;
    };

    match exclude_set {
        Some(set) => set.is_match(relative),
        None => false,
    }
}

fn is_supported_source_file(path: &Path, allow_js: bool) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    matches!(ext, "ts" | "tsx" | "mts" | "cts")
        || (allow_js && matches!(ext, "js" | "jsx" | "mjs" | "cjs"))
}
