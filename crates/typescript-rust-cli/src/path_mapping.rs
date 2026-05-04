use std::collections::{HashMap, HashSet};
use std::path::Path;

use typescript_rust_checker::SourceFileInput;
use typescript_rust_config::PathMapping;
use typescript_rust_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

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
        loaded_files.insert(normalize_path_string(&input.file_name), input.file_name.clone());
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
                ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
                    module_specifier: Some(module_specifier),
                    ..
                })
                | ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
                    module_specifier,
                    ..
                }) => {
                    if is_external_specifier(&module_specifier) {
                        specifiers_to_resolve.insert(module_specifier.clone());
                    }
                }
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

fn normalize_path_string(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut segments = Vec::new();
    let is_absolute = path.starts_with('/');
    let mut drive_letter = "";

    // Handle Windows drive letters
    let path_to_split = if path.chars().nth(1) == Some(':') {
        drive_letter = &path[0..2];
        &path[2..]
    } else {
        &path
    };

    for segment in path_to_split.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            if let Some(last) = segments.last() {
                if last != &".." {
                    segments.pop();
                    continue;
                }
            }
            if !is_absolute && drive_letter.is_empty() {
                segments.push(segment);
            }
            continue;
        }
        segments.push(segment);
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
            "/".to_string()
        } else {
            ".".to_string()
        }
    } else {
        result
    }
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
                let normalized = normalize_path_string(&joined.to_string_lossy());
                
                // Try candidate extensions
                let candidates = vec![
                    normalized.clone(),
                    format!("{}.ts", normalized),
                    format!("{}.d.ts", normalized),
                    format!("{}/index.ts", normalized),
                    format!("{}/index.d.ts", normalized),
                ];

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
