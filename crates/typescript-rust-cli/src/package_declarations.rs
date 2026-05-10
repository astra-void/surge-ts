use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use typescript_rust_checker::SourceFileInput;
use typescript_rust_config::canonicalize_if_exists;
use typescript_rust_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

pub struct PackageDeclarationRequest {
    pub specifier: String,
    pub package_name: String,
    pub subpath: Option<String>,
    pub importer_dir: PathBuf,
}

fn is_external_specifier(specifier: &str) -> bool {
    !specifier.starts_with("./")
        && !specifier.starts_with("../")
        && !specifier.starts_with(".\\")
        && !specifier.starts_with("..\\")
}

fn parse_package_specifier(specifier: &str) -> Option<(String, Option<String>)> {
    if specifier.starts_with('@') {
        let parts: Vec<&str> = specifier.splitn(3, '/').collect();
        if parts.len() >= 2 {
            let pkg_name = format!("{}/{}", parts[0], parts[1]);
            let subpath = if parts.len() == 3 {
                Some(parts[2].to_string())
            } else {
                None
            };
            Some((pkg_name, subpath))
        } else {
            None
        }
    } else {
        let mut parts = specifier.splitn(2, '/');
        if let Some(pkg_name) = parts.next() {
            let subpath = parts.next().map(|s| s.to_string());
            Some((pkg_name.to_string(), subpath))
        } else {
            None
        }
    }
}

fn resolve_exports_types(exports: &serde_json::Value, subpath_key: &str) -> Option<String> {
    if subpath_key.contains('*') {
        return None;
    }

    match exports {
        serde_json::Value::Object(map) => {
            if let Some(val) = map.get(subpath_key) {
                match val {
                    serde_json::Value::String(s) if is_declaration_file_path_str(s) => {
                        Some(s.clone())
                    }
                    serde_json::Value::Object(obj) => obj
                        .get("types")
                        .and_then(|types_val| types_val.as_str())
                        .filter(|s| is_declaration_file_path_str(s))
                        .map(|s| s.to_string()),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn resolve_package_declaration_entrypoints(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut packages_to_resolve: VecDeque<PackageDeclarationRequest> = VecDeque::new();
    let mut resolved_packages = HashMap::new();
    let mut known_file_names: HashSet<String> = inputs
        .iter()
        .map(|input| normalize_existing_path(&input.file_name))
        .collect();
    let mut queued_specifiers: HashSet<String> = HashSet::new();

    // Extract packages from initial inputs
    for (file_path, _, source_text) in sources.iter() {
        let importer_dir = file_path.parent().unwrap_or(root_dir).to_path_buf();
        extract_packages_from_source(
            source_text,
            &file_path.to_string_lossy(),
            &importer_dir,
            &mut packages_to_resolve,
            &mut queued_specifiers,
        );
    }

    // Bound the work queue to prevent any infinite loops despite deduping
    let mut max_resolutions = 1000;

    while let Some(req) = packages_to_resolve.pop_front() {
        if max_resolutions == 0 {
            break;
        }
        max_resolutions -= 1;

        if resolved_packages.contains_key(&req.specifier) {
            continue;
        }

        let mut current_dir = req.importer_dir.clone();
        let mut resolved_path = None;
        let mut runtime_only_path = None;

        loop {
            let pkg_dir = current_dir.join("node_modules").join(&req.package_name);
            let pkg_json_path = pkg_dir.join("package.json");

            if pkg_json_path.exists() && pkg_json_path.is_file() {
                if let Ok(json_str) = std::fs::read_to_string(&pkg_json_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        if let Some(subpath) = &req.subpath {
                            // Subpath resolution
                            let subpath_key = format!("./{}", subpath);

                            if let Some(exports) = json.get("exports") {
                                if let Some(types_path_str) =
                                    resolve_exports_types(exports, &subpath_key)
                                {
                                    let path = pkg_dir.join(types_path_str);
                                    if path.exists() && path.is_file() {
                                        resolved_path = Some(path);
                                        break;
                                    }
                                }
                            }
                        } else {
                            // Bare package resolution
                            let mut types_path = None;

                            if let Some(exports) = json.get("exports") {
                                if let Some(types_path_str) = resolve_exports_types(exports, ".") {
                                    types_path = Some(pkg_dir.join(types_path_str));
                                }
                            }

                            if types_path.is_none() {
                                if let Some(types) = json.get("types").and_then(|t| t.as_str()) {
                                    types_path = Some(pkg_dir.join(types));
                                } else if let Some(typings) =
                                    json.get("typings").and_then(|t| t.as_str())
                                {
                                    types_path = Some(pkg_dir.join(typings));
                                }
                            }

                            if let Some(path) = types_path {
                                let path = resolve_declaration_candidate(&path);

                                if path.is_some() {
                                    resolved_path = path;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // Fallbacks still apply when there is no package.json.
            if let Some(subpath) = &req.subpath {
                for candidate in declaration_candidates(pkg_dir.join(subpath)) {
                    if candidate.exists() && candidate.is_file() {
                        resolved_path = Some(candidate);
                        break;
                    }
                }
                if resolved_path.is_some() {
                    break;
                }

                for candidate in runtime_javascript_candidates(pkg_dir.join(subpath)) {
                    if candidate.exists() && candidate.is_file() {
                        runtime_only_path = Some(candidate);
                        break;
                    }
                }
                if runtime_only_path.is_some() {
                    break;
                }
            } else {
                for candidate in declaration_candidates(pkg_dir.join("index")) {
                    if candidate.exists() && candidate.is_file() {
                        resolved_path = Some(candidate);
                        break;
                    }
                }
                if resolved_path.is_some() {
                    break;
                }

                for candidate in runtime_javascript_candidates(pkg_dir.join("index")) {
                    if candidate.exists() && candidate.is_file() {
                        runtime_only_path = Some(candidate);
                        break;
                    }
                }
                if runtime_only_path.is_some() {
                    break;
                }
            }

            if let Some(parent) = current_dir.parent() {
                current_dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        if let Some(path) = resolved_path {
            if let Ok(path) = path.canonicalize() {
                let file_name = path.to_string_lossy().into_owned();

                let normalized_file_name = normalize_existing_path(&file_name);
                resolved_packages.insert(req.specifier.clone(), normalized_file_name.clone());

                if !known_file_names.contains(&normalized_file_name) {
                    if let Ok(source_text) = std::fs::read_to_string(&path) {
                        known_file_names.insert(normalized_file_name.clone());
                        inputs.push(SourceFileInput {
                            file_name: normalized_file_name.clone(),
                            source_text: source_text.clone(),
                        });
                        sources.push((path.clone(), normalized_file_name.clone(), source_text.clone()));

                        let new_importer_dir = path.parent().unwrap_or(root_dir).to_path_buf();
                        extract_packages_from_source(
                            &source_text,
                            &normalized_file_name,
                            &new_importer_dir,
                            &mut packages_to_resolve,
                            &mut queued_specifiers,
                        );
                    }
                }
            }
        } else if let Some(path) = runtime_only_path {
            if let Ok(path) = path.canonicalize() {
                let file_name = normalize_existing_path(&path.to_string_lossy());
                resolved_packages.insert(req.specifier.clone(), file_name);
            }
        }
    }

    resolved_packages
}

fn resolve_declaration_candidate(path: &Path) -> Option<PathBuf> {
    if is_declaration_file_path(path) && path.exists() && path.is_file() {
        return Some(path.to_path_buf());
    }

    for candidate in declaration_candidates(path.to_path_buf()) {
        if candidate.exists() && candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn declaration_candidates(path: PathBuf) -> Vec<PathBuf> {
    if is_declaration_file_path(&path) {
        return vec![path];
    }

    let declaration_stem = if is_runtime_javascript_file(&path) {
        path.with_extension("")
    } else if path.extension().is_none() {
        path
    } else {
        return Vec::new();
    };

    vec![
        declaration_stem.with_extension("d.ts"),
        declaration_stem.with_extension("d.mts"),
        declaration_stem.with_extension("d.cts"),
    ]
}

fn is_declaration_file_path_str(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

fn is_declaration_file_path(path: &Path) -> bool {
    is_declaration_file_path_str(&path.to_string_lossy())
}

fn is_runtime_javascript_file(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
}

fn normalize_existing_path(path: &str) -> String {
    canonicalize_if_exists(Path::new(path))
        .to_string_lossy()
        .replace('\\', "/")
}

fn runtime_javascript_candidates(path: PathBuf) -> Vec<PathBuf> {
    if is_runtime_javascript_file(&path) {
        return vec![path];
    }

    if path.extension().is_none() {
        return vec![
            path.with_extension("js"),
            path.with_extension("jsx"),
            path.with_extension("mjs"),
            path.with_extension("cjs"),
        ];
    }

    Vec::new()
}

fn extract_packages_from_source(
    source_text: &str,
    file_name: &str,
    importer_dir: &Path,
    packages_to_resolve: &mut VecDeque<PackageDeclarationRequest>,
    queued_specifiers: &mut HashSet<String>,
) {
    let parsed = parse_source(source_text, file_name);
    for statement in parsed.statements {
        match statement {
            ParsedStatement::ImportDeclaration(import) => {
                if is_external_specifier(&import.module_specifier) {
                    if !queued_specifiers.contains(&import.module_specifier) {
                        if let Some((package_name, subpath)) =
                            parse_package_specifier(&import.module_specifier)
                        {
                            queued_specifiers.insert(import.module_specifier.clone());
                            packages_to_resolve.push_back(PackageDeclarationRequest {
                                specifier: import.module_specifier.clone(),
                                package_name,
                                subpath,
                                importer_dir: importer_dir.to_path_buf(),
                            });
                        }
                    }
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
                    if !queued_specifiers.contains(&module_specifier) {
                        if let Some((package_name, subpath)) =
                            parse_package_specifier(&module_specifier)
                        {
                            queued_specifiers.insert(module_specifier.clone());
                            packages_to_resolve.push_back(PackageDeclarationRequest {
                                specifier: module_specifier.clone(),
                                package_name,
                                subpath,
                                importer_dir: importer_dir.to_path_buf(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_specifier() {
        assert_eq!(
            parse_package_specifier("pkg"),
            Some(("pkg".to_string(), None))
        );
        assert_eq!(
            parse_package_specifier("pkg/subpath"),
            Some(("pkg".to_string(), Some("subpath".to_string())))
        );
        assert_eq!(
            parse_package_specifier("pkg/nested/path"),
            Some(("pkg".to_string(), Some("nested/path".to_string())))
        );
        assert_eq!(
            parse_package_specifier("@scope/pkg"),
            Some(("@scope/pkg".to_string(), None))
        );
        assert_eq!(
            parse_package_specifier("@scope/pkg/helpers"),
            Some(("@scope/pkg".to_string(), Some("helpers".to_string())))
        );
        assert_eq!(
            parse_package_specifier("@scope/pkg/a/b"),
            Some(("@scope/pkg".to_string(), Some("a/b".to_string())))
        );
        assert_eq!(parse_package_specifier("@broken"), None);
    }

    #[test]
    fn test_resolve_exports_types() {
        let exports = serde_json::json!({
            ".": { "types": "./dist/index.d.ts" },
            "./feature": { "types": "./dist/feature.d.ts" },
            "./nested/path": { "types": "./dist/nested/path.d.ts" },
            "./string-dts": "./dist/string-dts.d.ts",
            "./runtime-only": "./dist/runtime.js",
            "./feature-nested": { "import": { "types": "./dist/feature.d.ts" } },
            "./wild/*": { "types": "./dist/*.d.ts" }
        });

        assert_eq!(
            resolve_exports_types(&exports, "."),
            Some("./dist/index.d.ts".to_string())
        );
        assert_eq!(
            resolve_exports_types(&exports, "./feature"),
            Some("./dist/feature.d.ts".to_string())
        );
        assert_eq!(
            resolve_exports_types(&exports, "./nested/path"),
            Some("./dist/nested/path.d.ts".to_string())
        );
        assert_eq!(
            resolve_exports_types(&exports, "./string-dts"),
            Some("./dist/string-dts.d.ts".to_string())
        );
        assert_eq!(resolve_exports_types(&exports, "./runtime-only"), None);
        assert_eq!(resolve_exports_types(&exports, "./feature-nested"), None);
        assert_eq!(resolve_exports_types(&exports, "./wild/*"), None);
        assert_eq!(resolve_exports_types(&exports, "./wild/feature"), None);
        assert_eq!(resolve_exports_types(&exports, "./missing"), None);
    }

    #[test]
    fn test_declaration_candidates_skip_runtime_js() {
        let candidates = declaration_candidates(PathBuf::from("pkg/subpath.js"));
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("pkg/subpath.d.ts"),
                PathBuf::from("pkg/subpath.d.mts"),
                PathBuf::from("pkg/subpath.d.cts"),
            ]
        );
    }

    #[test]
    fn test_declaration_candidates_keep_declaration_files() {
        let candidates = declaration_candidates(PathBuf::from("pkg/subpath.d.ts"));
        assert_eq!(candidates, vec![PathBuf::from("pkg/subpath.d.ts")]);
    }
}
