use std::collections::{HashMap, HashSet};
use std::path::Path;

use surge_ts_checker::SourceFileInput;
use surge_ts_config::PathMapping;
use surge_ts_config::canonicalize_if_exists_string;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

pub fn resolve_path_mappings(
    inputs: &[SourceFileInput],
    paths: &[PathMapping],
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut resolved_modules = HashMap::new();
    if paths.is_empty() {
        return resolved_modules;
    }

    // Build lookup of loaded files (normalized path -> original path)
    let mut loaded_files = HashMap::new();
    for input in inputs {
        loaded_files.insert(
            canonicalize_if_exists_string(Path::new(&input.file_name)),
            input.file_name.clone(),
        );
    }

    let mut specifiers_to_resolve = HashSet::new();

    for input in inputs {
        let parsed = parse_source(&input.source_text, &input.file_name);
        for statement in parsed.statements {
            match statement {
                ParsedStatement::ImportDeclaration(import) => {
                    if is_external_specifier(&import.module_specifier) {
                        specifiers_to_resolve.insert(import.module_specifier.clone());
                    }
                }
                ParsedStatement::ExportDeclaration(export) => match *export {
                    ParsedExportDeclaration::Named {
                        module_specifier: Some(module_specifier),
                        ..
                    }
                    | ParsedExportDeclaration::All {
                        module_specifier, ..
                    } => {
                        if is_external_specifier(&module_specifier) {
                            specifiers_to_resolve.insert(module_specifier.clone());
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    for specifier in specifiers_to_resolve {
        if let Some(resolved_path) =
            try_resolve_path_mapping(&specifier, paths, root_dir, &loaded_files)
        {
            resolved_modules.insert(specifier, resolved_path);
        }
    }

    resolved_modules
}

fn is_external_specifier(specifier: &str) -> bool {
    !specifier.starts_with("./")
        && !specifier.starts_with("../")
        && !specifier.starts_with(".\\")
        && !specifier.starts_with("..\\")
}

fn try_resolve_path_mapping(
    specifier: &str,
    paths: &[PathMapping],
    root_dir: &Path,
    loaded_files: &HashMap<String, String>,
) -> Option<String> {
    for mapping in paths {
        let is_wildcard = mapping.pattern.contains('*');

        // For now, only exact or single-star patterns are supported
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
        } else {
            if specifier == mapping.pattern {
                Some("")
            } else {
                None
            }
        };

        if let Some(matched_text) = matched_text {
            for substitution in &mapping.substitutions {
                if substitution.matches('*').count() > 1 {
                    continue;
                }

                let target_path = if is_wildcard {
                    substitution.replace('*', matched_text)
                } else {
                    substitution.clone()
                };

                // The target_path is relative to root_dir
                let joined = root_dir.join(&target_path);

                // Normalize path to use forward slashes
                let normalized = canonicalize_if_exists_string(&joined);

                let candidates = path_resolution_candidates(&normalized);

                for candidate in candidates {
                    if let Some(original_name) = loaded_files.get(&candidate) {
                        return Some(original_name.clone());
                    }
                }
            }
        }
    }

    None
}

fn path_resolution_candidates(base: &str) -> Vec<String> {
    let lower = base.to_ascii_lowercase();

    if lower.ends_with(".js") {
        let stem = strip_extension(base);
        return vec![
            format!("{stem}.ts"),
            format!("{stem}.tsx"),
            format!("{stem}.mts"),
            format!("{stem}.cts"),
            format!("{stem}.d.ts"),
            format!("{stem}.d.mts"),
            format!("{stem}.d.cts"),
        ];
    }

    if lower.ends_with(".mjs") {
        let stem = strip_extension(base);
        return vec![format!("{stem}.mts"), format!("{stem}.d.mts")];
    }

    if lower.ends_with(".cjs") {
        let stem = strip_extension(base);
        return vec![format!("{stem}.cts"), format!("{stem}.d.cts")];
    }

    if lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".mts")
        || lower.ends_with(".cts")
        || lower.ends_with(".d.ts")
        || lower.ends_with(".d.mts")
        || lower.ends_with(".d.cts")
    {
        return vec![base.to_string()];
    }

    vec![
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.mts"),
        format!("{base}.cts"),
        format!("{base}.d.ts"),
        format!("{base}.d.mts"),
        format!("{base}.d.cts"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.mts"),
        format!("{base}/index.cts"),
        format!("{base}/index.d.ts"),
        format!("{base}/index.d.mts"),
        format!("{base}/index.d.cts"),
    ]
}

fn strip_extension(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}
