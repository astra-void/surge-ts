use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use surge_ts_checker::SourceFileInput;
use surge_ts_config::PathMapping;
use surge_ts_config::{
    canonicalize_if_exists, canonicalize_if_exists_string, normalize_path_string,
};
use surge_ts_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

pub fn expand_project_inputs(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    paths: &[PathMapping],
) -> usize {
    let mut known_files = HashSet::new();
    for input in inputs.iter() {
        known_files.insert(canonicalize_if_exists_string(Path::new(&input.file_name)));
    }

    let mut added = 0usize;
    let mut index = 0usize;
    let mut probe_cache: HashMap<String, bool> = HashMap::new();

    while index < sources.len() {
        let (file_path, file_name, source_text) = {
            let (file_path, file_name, source_text) = &sources[index];
            (file_path.clone(), file_name.clone(), source_text.clone())
        };
        index += 1;

        let parsed = parse_source(&source_text, &file_name);
        for statement in parsed.statements {
            let Some(module_specifier) = module_specifier_from_statement(&statement) else {
                continue;
            };

            let candidate = if is_relative_specifier(&module_specifier) {
                resolve_relative_candidate(&file_path, &module_specifier, &mut probe_cache)
            } else {
                resolve_paths_alias_candidate(&module_specifier, paths, root_dir, &mut probe_cache)
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
            if !known_files.insert(normalized) {
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

    added
}

fn module_specifier_from_statement(statement: &ParsedStatement) -> Option<String> {
    match statement {
        ParsedStatement::ImportDeclaration(import) => Some(import.module_specifier.clone()),
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Named {
                module_specifier: Some(module_specifier),
                ..
            } => Some(module_specifier.clone()),
            ParsedExportDeclaration::All {
                module_specifier, ..
            } => Some(module_specifier.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_relative_candidate(
    importer_file: &Path,
    specifier: &str,
    probe_cache: &mut HashMap<String, bool>,
) -> Option<PathBuf> {
    let importer_dir = importer_file.parent().unwrap_or_else(|| Path::new(""));
    let normalized_specifier = normalize_path_string(specifier);
    let joined = normalize_path_string(&importer_dir.join(&normalized_specifier).to_string_lossy());

    let candidate_paths = match relative_specifier_kind(&normalized_specifier) {
        RelativeSpecifierKind::ExplicitTs => vec![joined],
        RelativeSpecifierKind::ExplicitJs => {
            let mut candidates = vec![joined.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined),
                &[".ts", ".tsx"],
                &[".d.ts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitMjs => {
            let mut candidates = vec![joined.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined),
                &[".mts"],
                &[".d.mts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitCjs => {
            let mut candidates = vec![joined.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined),
                &[".cts"],
                &[".d.cts"],
            ));
            candidates
        }
        RelativeSpecifierKind::Extensionless => relative_resolution_candidates(&joined),
        RelativeSpecifierKind::Unsupported => return None,
    };

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
    root_dir: &Path,
    probe_cache: &mut HashMap<String, bool>,
) -> Option<PathBuf> {
    for mapping in paths {
        let is_wildcard = mapping.pattern.contains('*');
        if mapping.pattern.matches('*').count() > 1 {
            continue;
        }

        let matched_text = if is_wildcard {
            let parts: Vec<&str> = mapping.pattern.split('*').collect();
            if parts.len() != 2 {
                continue;
            }

            let prefix = parts[0];
            let suffix = parts[1];
            if specifier.starts_with(prefix)
                && specifier.ends_with(suffix)
                && specifier.len() >= prefix.len() + suffix.len()
            {
                let start = prefix.len();
                let end = specifier.len() - suffix.len();
                Some(&specifier[start..end])
            } else {
                None
            }
        } else if specifier == mapping.pattern {
            Some("")
        } else {
            None
        };

        let Some(matched_text) = matched_text else {
            continue;
        };

        for substitution in &mapping.substitutions {
            if substitution.matches('*').count() > 1 {
                continue;
            }

            let target_path = if is_wildcard {
                substitution.replace('*', matched_text)
            } else {
                substitution.clone()
            };

            if !is_explicit_relative_target(&target_path) {
                continue;
            }

            let joined = normalize_path_string(&root_dir.join(&target_path).to_string_lossy());
            let candidate_paths = if joined.contains('.') {
                // Preserve the existing candidate policy for explicit relative targets.
                relative_resolution_candidates(&joined)
            } else {
                relative_resolution_candidates(&joined)
            };

            for candidate in candidate_paths {
                let candidate = PathBuf::from(candidate);
                if !candidate_is_existing_file(&candidate, probe_cache) {
                    continue;
                }

                if is_dependency_javascript_source_file(&candidate)
                    || !is_loadable_graph_file(&candidate)
                {
                    continue;
                }

                return Some(candidate);
            }
        }
    }

    None
}

// Each probe previously issued two stat syscalls (`exists()` then `is_file()`).
// A single `metadata` call answers both, and most extensionless specifiers fan
// out to ~15 candidate paths, so caching by path collapses repeated probes for
// modules imported from many files.
fn candidate_is_existing_file(candidate: &Path, cache: &mut HashMap<String, bool>) -> bool {
    let key = candidate.to_string_lossy();
    if let Some(&hit) = cache.get(key.as_ref()) {
        return hit;
    }
    crate::io_stats::record_existence_probe();
    let is_file = fs::metadata(candidate)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    cache.insert(key.into_owned(), is_file);
    is_file
}

fn is_loadable_graph_file(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    if lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts") {
        return true;
    }

    if lower.contains("/node_modules/") || lower.contains("\\node_modules\\") {
        return false;
    }

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

fn is_explicit_relative_target(target: &str) -> bool {
    target.starts_with("./")
        || target.starts_with("../")
        || target.starts_with(".\\")
        || target.starts_with("..\\")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeSpecifierKind {
    ExplicitTs,
    ExplicitJs,
    ExplicitMjs,
    ExplicitCjs,
    Extensionless,
    Unsupported,
}

fn relative_specifier_kind(specifier: &str) -> RelativeSpecifierKind {
    let last_segment = specifier.rsplit('/').next().unwrap_or(specifier);

    if last_segment == "." || last_segment == ".." {
        return RelativeSpecifierKind::Extensionless;
    }

    if last_segment.ends_with(".tsx")
        || last_segment.ends_with(".jsx")
        || last_segment.ends_with(".mts")
        || last_segment.ends_with(".cts")
        || last_segment.ends_with(".d.ts")
        || last_segment.ends_with(".d.mts")
        || last_segment.ends_with(".d.cts")
        || last_segment.ends_with(".json")
    {
        return RelativeSpecifierKind::Unsupported;
    }

    if last_segment.ends_with(".ts") {
        return RelativeSpecifierKind::ExplicitTs;
    }

    if last_segment.ends_with(".js") {
        return RelativeSpecifierKind::ExplicitJs;
    }

    if last_segment.ends_with(".mjs") {
        return RelativeSpecifierKind::ExplicitMjs;
    }

    if last_segment.ends_with(".cjs") {
        return RelativeSpecifierKind::ExplicitCjs;
    }

    RelativeSpecifierKind::Extensionless
}

fn relative_resolution_candidates(base: &str) -> Vec<String> {
    vec![
        base.to_string(),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.d.ts"),
        format!("{base}.mts"),
        format!("{base}.cts"),
        format!("{base}.d.mts"),
        format!("{base}.d.cts"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.d.ts"),
        format!("{base}/index.mts"),
        format!("{base}/index.cts"),
        format!("{base}/index.d.mts"),
        format!("{base}/index.d.cts"),
    ]
}

fn relative_resolution_candidates_with_js_substitution(
    base: &str,
    source_extensions: &[&str],
    declaration_extensions: &[&str],
) -> Vec<String> {
    let mut candidates = Vec::new();

    for extension in source_extensions {
        candidates.push(format!("{base}{extension}"));
    }
    for extension in declaration_extensions {
        candidates.push(format!("{base}{extension}"));
    }

    candidates.push(format!("{base}/index.ts"));
    candidates.push(format!("{base}/index.tsx"));
    candidates.push(format!("{base}/index.d.ts"));
    candidates.push(format!("{base}/index.mts"));
    candidates.push(format!("{base}/index.cts"));
    candidates.push(format!("{base}/index.d.mts"));
    candidates.push(format!("{base}/index.d.cts"));

    candidates
}

fn strip_extension(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}
