use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use typescript_rust_checker::SourceFileInput;
use typescript_rust_syntax::{parse_source, ParsedExportDeclaration, ParsedStatement};

fn is_external_specifier(specifier: &str) -> bool {
    !specifier.starts_with("./")
        && !specifier.starts_with("../")
        && !specifier.starts_with(".\\")
        && !specifier.starts_with("..\\")
}

fn extract_package_name(specifier: &str) -> Option<String> {
    if specifier.starts_with('@') {
        let parts: Vec<&str> = specifier.split('/').collect();
        if parts.len() >= 2 {
            Some(format!("{}/{}", parts[0], parts[1]))
        } else {
            None
        }
    } else {
        specifier.split('/').next().map(|s| s.to_string())
    }
}

pub fn resolve_package_declaration_entrypoints(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut packages_to_resolve: VecDeque<(String, PathBuf)> = VecDeque::new();
    let mut resolved_packages = HashMap::new();
    let mut known_file_names: HashSet<String> = inputs.iter().map(|input| input.file_name.clone()).collect();
    
    // Extract packages from initial inputs
    for (file_path, _, source_text) in sources.iter() {
        let importer_dir = file_path.parent().unwrap_or(root_dir).to_path_buf();
        extract_packages_from_source(source_text, &file_path.to_string_lossy(), &importer_dir, &mut packages_to_resolve);
    }
    
    // Bound the work queue to prevent any infinite loops despite deduping
    let mut max_resolutions = 1000;
    
    while let Some((pkg, importer_dir)) = packages_to_resolve.pop_front() {
        if max_resolutions == 0 {
            break;
        }
        max_resolutions -= 1;
        
        if resolved_packages.contains_key(&pkg) {
            continue;
        }
        
        let mut current_dir = importer_dir.clone();
        let mut resolved_path = None;
        
        loop {
            let pkg_dir = current_dir.join("node_modules").join(&pkg);
            let pkg_json_path = pkg_dir.join("package.json");
            
            if pkg_json_path.exists() && pkg_json_path.is_file() {
                if let Ok(json_str) = std::fs::read_to_string(&pkg_json_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        let mut types_path = None;
                        
                        if let Some(types) = json.get("types").and_then(|t| t.as_str()) {
                            types_path = Some(pkg_dir.join(types));
                        } else if let Some(typings) = json.get("typings").and_then(|t| t.as_str()) {
                            types_path = Some(pkg_dir.join(typings));
                        }
                        
                        if let Some(path) = types_path {
                            let path = if path.exists() && path.is_file() {
                                Some(path)
                            } else if path.extension().is_none() && path.with_extension("d.ts").exists() {
                                Some(path.with_extension("d.ts"))
                            } else {
                                None
                            };
                            
                            if path.is_some() {
                                resolved_path = path;
                                break;
                            }
                        }
                    }
                }
                
                // Fallback to index.d.ts
                let index_dts = pkg_dir.join("index.d.ts");
                if index_dts.exists() && index_dts.is_file() {
                    resolved_path = Some(index_dts);
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
                
                resolved_packages.insert(pkg.clone(), file_name.clone());
                
                if !known_file_names.contains(&file_name) {
                    if let Ok(source_text) = std::fs::read_to_string(&path) {
                        known_file_names.insert(file_name.clone());
                        inputs.push(SourceFileInput {
                            file_name: file_name.clone(),
                            source_text: source_text.clone(),
                        });
                        sources.push((path.clone(), file_name.clone(), source_text.clone()));
                        
                        let new_importer_dir = path.parent().unwrap_or(root_dir).to_path_buf();
                        extract_packages_from_source(&source_text, &file_name, &new_importer_dir, &mut packages_to_resolve);
                    }
                }
            }
        }
    }
    
    resolved_packages
}

fn extract_packages_from_source(
    source_text: &str,
    file_name: &str,
    importer_dir: &Path,
    packages_to_resolve: &mut VecDeque<(String, PathBuf)>,
) {
    let parsed = parse_source(source_text, file_name);
    for statement in parsed.statements {
        match statement {
            ParsedStatement::ImportDeclaration(import) => {
                if is_external_specifier(&import.module_specifier) {
                    if let Some(pkg) = extract_package_name(&import.module_specifier) {
                        if pkg == import.module_specifier {
                            packages_to_resolve.push_back((pkg, importer_dir.to_path_buf()));
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
                    if let Some(pkg) = extract_package_name(&module_specifier) {
                        if pkg == module_specifier {
                            packages_to_resolve.push_back((pkg, importer_dir.to_path_buf()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
