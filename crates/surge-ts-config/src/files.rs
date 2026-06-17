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

    let include_set = build_globset(&include_patterns, diagnostics, root_dir);
    let exclude_set = build_globset(&exclude_patterns, diagnostics, root_dir);

    let mut files = Vec::new();
    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_prune(entry.path(), root_dir, exclude_set.as_ref()))
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
            if !set.is_match(relative) && !is_under_any_include_root(relative, &include_roots) {
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

fn is_under_any_include_root(relative: &Path, include_roots: &[PathBuf]) -> bool {
    include_roots.iter().any(|root| relative.starts_with(root))
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
