use std::collections::{HashMap, HashSet};
use std::path::Path;

use surge_ts_checker::SourceFileInput;
use surge_ts_config::PathMapping;
use surge_ts_config::canonicalize_if_exists_string;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

pub fn resolve_path_mappings(
    inputs: &[SourceFileInput],
    paths: &[PathMapping],
    base_url: Option<&Path>,
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut resolved_modules = HashMap::new();
    if paths.is_empty() && base_url.is_none() {
        return resolved_modules;
    }

    // `paths` substitutions and the bare-import fallback are both anchored at
    // `baseUrl` (matching `tsc`); when `baseUrl` is unset, `paths` resolves
    // relative to the config directory.
    let mapping_base = base_url.unwrap_or(root_dir);

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
            try_resolve_path_mapping(&specifier, paths, mapping_base, &loaded_files)
        {
            resolved_modules.insert(specifier, resolved_path);
            continue;
        }

        // tsc falls back to resolving a non-relative specifier directly against
        // `baseUrl` when no `paths` pattern matched.
        if let Some(base_url) = base_url {
            if let Some(resolved_path) = try_resolve_base_url(&specifier, base_url, &loaded_files) {
                resolved_modules.insert(specifier, resolved_path);
            }
        }
    }

    resolved_modules
}

fn try_resolve_base_url(
    specifier: &str,
    base_url: &Path,
    loaded_files: &HashMap<String, String>,
) -> Option<String> {
    let joined = base_url.join(specifier);
    let normalized = canonicalize_if_exists_string(&joined);
    for candidate in path_resolution_candidates(&normalized) {
        if let Some(original_name) = loaded_files.get(&candidate) {
            return Some(original_name.clone());
        }
    }
    None
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
    mapping_base: &Path,
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

                // `paths` substitutions are relative to `baseUrl` (or the config
                // directory when `baseUrl` is unset).
                let joined = mapping_base.join(&target_path);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn scratch_dir() -> PathBuf {
        let unique = format!(
            "surge-base-url-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).unwrap();
        // Canonicalize so the directory matches the resolver's canonicalized
        // file keys (e.g. macOS `/var` -> `/private/var`).
        fs::canonicalize(&dir).unwrap()
    }

    fn write(dir: &Path, rel: &str, body: &str) -> String {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path.to_string_lossy().replace('\\', "/")
    }

    fn input(file_name: &str, source_text: &str) -> SourceFileInput {
        SourceFileInput {
            file_name: file_name.to_string(),
            source_text: source_text.to_string(),
        }
    }

    fn canonical(file_name: &str) -> String {
        canonicalize_if_exists_string(Path::new(file_name))
    }

    // `baseUrl: src` anchors both `paths` substitutions and the bare-import
    // fallback, matching how roblox-ts (and other `baseUrl` projects) resolve.
    #[test]
    fn base_url_resolves_paths_substitution_and_bare_import_fallback() {
        let root = scratch_dir();
        let base_url = root.join("src");

        let constants = write(&root, "src/shared/constants.ts", "export const X = 1;");
        let util = write(&root, "src/runtime/util.ts", "export const U = 2;");
        let index_src =
            "import { X } from \"shared/constants\";\nimport { U } from \"@runtime/util\";\n";
        let index_path = write(&root, "src/index.ts", index_src);

        let inputs = vec![
            input(&index_path, index_src),
            input(&constants, "export const X = 1;"),
            input(&util, "export const U = 2;"),
        ];

        let paths = vec![PathMapping {
            pattern: "@runtime/*".to_string(),
            substitutions: vec!["runtime/*".to_string()],
        }];

        let resolved = resolve_path_mappings(&inputs, &paths, Some(base_url.as_path()), &root);

        assert_eq!(
            resolved.get("shared/constants").map(String::as_str),
            Some(canonical(&constants).as_str())
        );
        assert_eq!(
            resolved.get("@runtime/util").map(String::as_str),
            Some(canonical(&util).as_str())
        );
    }

    // Without `baseUrl`, a non-relative bare specifier has no anchor and stays
    // unresolved (only relative imports and `paths` patterns resolve).
    #[test]
    fn bare_import_does_not_resolve_without_base_url() {
        let root = scratch_dir();
        let constants = write(&root, "shared/constants.ts", "export const X = 1;");
        let index_src = "import { X } from \"shared/constants\";\n";
        let index_path = write(&root, "index.ts", index_src);

        let inputs = vec![
            input(&index_path, index_src),
            input(&constants, "export const X = 1;"),
        ];

        // `paths` empty + no `baseUrl`: the early return means nothing resolves,
        // even though the target file is loaded.
        let resolved = resolve_path_mappings(&inputs, &[], None, &root);
        assert!(resolved.get("shared/constants").is_none());
    }
}
