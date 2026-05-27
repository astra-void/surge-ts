use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use typescript_rust_checker::SourceFileInput;
use typescript_rust_config::canonicalize_if_exists_string;
use typescript_rust_syntax::{ParsedExportDeclaration, ParsedStatement, parse_source};

pub struct PackageDeclarationRequest {
    pub specifier: String,
    pub package_name: String,
    pub subpath: Option<String>,
    pub importer_dir: PathBuf,
}

#[derive(Debug, Default)]
pub(crate) struct PackageDeclarationResolverCache {
    package_json_cache: HashMap<PathBuf, Option<serde_json::Value>>,
    entrypoint_cache: HashMap<PackageEntrypointCacheKey, Option<PackageEntrypointResolution>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PackageEntrypointCacheKey {
    importer_dir: String,
    package_name: String,
    subpath: Option<String>,
}

#[derive(Debug, Clone)]
struct PackageEntrypointResolution {
    path: PathBuf,
    kind: PackageEntrypointKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageEntrypointKind {
    Declaration,
    RuntimeOnly,
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

#[allow(dead_code)]
fn resolve_exports_types(exports: &serde_json::Value, subpath_key: &str) -> Option<String> {
    if subpath_key.contains('*') {
        return None;
    }

    match exports {
        serde_json::Value::String(s) => {
            if is_declaration_file_path_str(s) {
                Some(s.clone())
            } else {
                None
            }
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_exports_types(item, subpath_key)),
        serde_json::Value::Object(map) => {
            if let Some(val) = map.get(subpath_key) {
                if let Some(types) = resolve_types_condition_value(val) {
                    return Some(types);
                }
            }

            if subpath_key == "." {
                if let Some(types) = map.get("types").and_then(resolve_types_condition_value) {
                    return Some(types);
                }
            }

            None
        }
        _ => None,
    }
}

fn resolve_exports_entrypoint(exports: &serde_json::Value, subpath_key: &str) -> Option<String> {
    if subpath_key.contains('*') {
        return None;
    }

    match exports {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_exports_entrypoint(item, subpath_key)),
        serde_json::Value::Object(map) => {
            if let Some(val) = map.get(subpath_key) {
                if let Some(path) = resolve_export_entrypoint_condition_value(val) {
                    return Some(path);
                }
            }

            if subpath_key == "." {
                if let Some(path) = resolve_export_entrypoint_condition_value(exports) {
                    return Some(path);
                }
            }

            None
        }
        _ => None,
    }
}

fn resolve_export_entrypoint_condition_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(resolve_export_entrypoint_condition_value),
        serde_json::Value::Object(map) => {
            if let Some(types) = map
                .get("types")
                .and_then(resolve_export_entrypoint_condition_value)
            {
                return Some(types);
            }

            for value in map.values() {
                if let Some(path) = resolve_export_entrypoint_condition_value(value) {
                    return Some(path);
                }
            }

            None
        }
        _ => None,
    }
}

#[allow(dead_code)]
fn resolve_types_condition_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            if is_declaration_file_path_str(s) {
                Some(s.clone())
            } else {
                None
            }
        }
        serde_json::Value::Array(items) => items.iter().find_map(resolve_types_condition_value),
        serde_json::Value::Object(map) => {
            if let Some(types) = map.get("types").and_then(resolve_types_condition_value) {
                return Some(types);
            }

            for value in map.values() {
                if let Some(types) = resolve_types_condition_value(value) {
                    return Some(types);
                }
            }

            None
        }
        _ => None,
    }
}

#[allow(dead_code)]
pub fn resolve_package_declaration_entrypoints(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut cache = PackageDeclarationResolverCache::default();
    resolve_package_declaration_entrypoints_with_cache(inputs, sources, root_dir, &mut cache)
}

pub(crate) fn resolve_package_declaration_entrypoints_with_cache(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> HashMap<String, String> {
    let mut packages_to_resolve: VecDeque<PackageDeclarationRequest> = VecDeque::new();
    let mut resolved_packages = HashMap::new();
    let mut known_file_names: HashSet<String> = inputs
        .iter()
        .map(|input| canonicalize_if_exists_string(Path::new(&input.file_name)))
        .collect();
    let mut queued_specifiers: HashSet<String> = HashSet::new();

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

    let mut max_resolutions = 1000;

    while let Some(req) = packages_to_resolve.pop_front() {
        if max_resolutions == 0 {
            break;
        }
        max_resolutions -= 1;

        if resolved_packages.contains_key(&req.specifier) {
            continue;
        }

        let cache_key = PackageEntrypointCacheKey {
            importer_dir: canonicalize_if_exists_string(&req.importer_dir),
            package_name: req.package_name.clone(),
            subpath: req.subpath.clone(),
        };

        let resolution = if let Some(cached) = cache.entrypoint_cache.get(&cache_key) {
            cached.clone()
        } else {
            let resolved = resolve_package_entrypoint(&req, cache, root_dir);
            cache.entrypoint_cache.insert(cache_key, resolved.clone());
            resolved
        };

        let Some(resolution) = resolution else {
            continue;
        };

        match resolution.kind {
            PackageEntrypointKind::Declaration => {
                let Ok(path) = resolution.path.canonicalize() else {
                    continue;
                };

                let normalized_file_name = canonicalize_if_exists_string(&path);
                resolved_packages.insert(req.specifier.clone(), normalized_file_name.clone());

                if !known_file_names.contains(&normalized_file_name) {
                    let Ok(source_text) = std::fs::read_to_string(&path) else {
                        continue;
                    };

                    known_file_names.insert(normalized_file_name.clone());
                    inputs.push(SourceFileInput {
                        file_name: normalized_file_name.clone(),
                        source_text: source_text.clone(),
                    });
                    sources.push((
                        path.clone(),
                        normalized_file_name.clone(),
                        source_text.clone(),
                    ));

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
            PackageEntrypointKind::RuntimeOnly => {
                if let Ok(path) = resolution.path.canonicalize() {
                    let file_name = canonicalize_if_exists_string(&path);
                    resolved_packages.insert(req.specifier.clone(), file_name);
                }
            }
        }
    }

    resolved_packages
}

fn resolve_package_entrypoint(
    req: &PackageDeclarationRequest,
    cache: &mut PackageDeclarationResolverCache,
    root_dir: &Path,
) -> Option<PackageEntrypointResolution> {
    let mut current_dir = req.importer_dir.clone();
    let mut runtime_fallback = None;

    loop {
        let pkg_dir = current_dir.join("node_modules").join(&req.package_name);

        if let Some(resolution) = resolve_package_entrypoint_in_directory(req, &pkg_dir, cache) {
            match resolution.kind {
                PackageEntrypointKind::Declaration => {
                    return Some(resolution);
                }
                PackageEntrypointKind::RuntimeOnly => {
                    if runtime_fallback.is_none() {
                        runtime_fallback = Some(resolution);
                    }
                }
            }
        }

        if let Some(resolution) =
            resolve_at_types_fallback_in_directory(req, &current_dir, root_dir)
        {
            return Some(resolution);
        }

        let Some(parent) = current_dir.parent() else {
            break;
        };
        current_dir = parent.to_path_buf();
    }

    runtime_fallback
}

fn resolve_package_entrypoint_in_directory(
    req: &PackageDeclarationRequest,
    pkg_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PackageEntrypointResolution> {
    let mut runtime_fallback = None;
    let pkg_json_path = pkg_dir.join("package.json");

    if pkg_json_path.exists() && pkg_json_path.is_file() {
        if let Some(json) = read_package_json(&pkg_json_path, cache) {
            if let Some(subpath) = &req.subpath {
                let subpath_key = format!("./{}", subpath);

                if let Some(exports) = json.get("exports") {
                    if let Some(path_str) = resolve_exports_entrypoint(exports, &subpath_key) {
                        let path = pkg_dir.join(path_str);
                        if let Some(resolution) = resolve_declaration_or_runtime_candidate(&path) {
                            match resolution.kind {
                                PackageEntrypointKind::Declaration => return Some(resolution),
                                PackageEntrypointKind::RuntimeOnly => {
                                    if runtime_fallback.is_none() {
                                        runtime_fallback = Some(resolution);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(resolution) =
                    resolve_declaration_or_runtime_candidate(&pkg_dir.join(subpath))
                {
                    match resolution.kind {
                        PackageEntrypointKind::Declaration => return Some(resolution),
                        PackageEntrypointKind::RuntimeOnly => {
                            if runtime_fallback.is_none() {
                                runtime_fallback = Some(resolution);
                            }
                        }
                    }
                }
            } else {
                if let Some(exports) = json.get("exports") {
                    if let Some(path_str) = resolve_exports_entrypoint(exports, ".") {
                        let path = pkg_dir.join(path_str);
                        if let Some(resolution) = resolve_declaration_or_runtime_candidate(&path) {
                            match resolution.kind {
                                PackageEntrypointKind::Declaration => return Some(resolution),
                                PackageEntrypointKind::RuntimeOnly => {
                                    if runtime_fallback.is_none() {
                                        runtime_fallback = Some(resolution);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(types) = json.get("types").and_then(|t| t.as_str()) {
                    if let Some(resolution) =
                        resolve_declaration_or_runtime_candidate(&pkg_dir.join(types))
                    {
                        match resolution.kind {
                            PackageEntrypointKind::Declaration => return Some(resolution),
                            PackageEntrypointKind::RuntimeOnly => {
                                if runtime_fallback.is_none() {
                                    runtime_fallback = Some(resolution);
                                }
                            }
                        }
                    }
                }

                if let Some(typings) = json.get("typings").and_then(|t| t.as_str()) {
                    if let Some(resolution) =
                        resolve_declaration_or_runtime_candidate(&pkg_dir.join(typings))
                    {
                        match resolution.kind {
                            PackageEntrypointKind::Declaration => return Some(resolution),
                            PackageEntrypointKind::RuntimeOnly => {
                                if runtime_fallback.is_none() {
                                    runtime_fallback = Some(resolution);
                                }
                            }
                        }
                    }
                }

                if let Some(module_path) = json.get("module").and_then(|t| t.as_str()) {
                    if let Some(resolution) =
                        resolve_declaration_or_runtime_candidate(&pkg_dir.join(module_path))
                    {
                        match resolution.kind {
                            PackageEntrypointKind::Declaration => return Some(resolution),
                            PackageEntrypointKind::RuntimeOnly => {
                                if runtime_fallback.is_none() {
                                    runtime_fallback = Some(resolution);
                                }
                            }
                        }
                    }
                }

                if let Some(main_path) = json.get("main").and_then(|t| t.as_str()) {
                    if let Some(resolution) =
                        resolve_declaration_or_runtime_candidate(&pkg_dir.join(main_path))
                    {
                        match resolution.kind {
                            PackageEntrypointKind::Declaration => return Some(resolution),
                            PackageEntrypointKind::RuntimeOnly => {
                                if runtime_fallback.is_none() {
                                    runtime_fallback = Some(resolution);
                                }
                            }
                        }
                    }
                }

                for candidate in [
                    "dist/types/index",
                    "types/index",
                    "typings/index",
                    "dist/esm/index",
                    "dist/index",
                ] {
                    if let Some(resolution) =
                        resolve_declaration_or_runtime_candidate(&pkg_dir.join(candidate))
                    {
                        match resolution.kind {
                            PackageEntrypointKind::Declaration => return Some(resolution),
                            PackageEntrypointKind::RuntimeOnly => {
                                if runtime_fallback.is_none() {
                                    runtime_fallback = Some(resolution);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(subpath) = &req.subpath {
        if let Some(resolution) = resolve_declaration_or_runtime_candidate(&pkg_dir.join(subpath)) {
            match resolution.kind {
                PackageEntrypointKind::Declaration => return Some(resolution),
                PackageEntrypointKind::RuntimeOnly => {
                    if runtime_fallback.is_none() {
                        runtime_fallback = Some(resolution);
                    }
                }
            }
        }
    } else if let Some(resolution) =
        resolve_declaration_or_runtime_candidate(&pkg_dir.join("index"))
    {
        match resolution.kind {
            PackageEntrypointKind::Declaration => return Some(resolution),
            PackageEntrypointKind::RuntimeOnly => {
                if runtime_fallback.is_none() {
                    runtime_fallback = Some(resolution);
                }
            }
        }
    }

    runtime_fallback
}

fn resolve_at_types_fallback_in_directory(
    req: &PackageDeclarationRequest,
    current_dir: &Path,
    root_dir: &Path,
) -> Option<PackageEntrypointResolution> {
    let fallback_name = types_package_name(&req.package_name);

    let fallback_dir = current_dir
        .join("node_modules")
        .join("@types")
        .join(&fallback_name);
    if let Some(resolution) = resolve_types_package_directory(&fallback_dir, req.subpath.as_deref())
    {
        return Some(resolution);
    }

    if current_dir != root_dir {
        let root_fallback_dir = root_dir
            .join("node_modules")
            .join("@types")
            .join(&fallback_name);
        if let Some(resolution) =
            resolve_types_package_directory(&root_fallback_dir, req.subpath.as_deref())
        {
            return Some(resolution);
        }
    }

    None
}

fn resolve_types_package_directory(
    package_dir: &Path,
    subpath: Option<&str>,
) -> Option<PackageEntrypointResolution> {
    if let Some(subpath) = subpath {
        if let Some(resolution) =
            resolve_declaration_or_runtime_candidate(&package_dir.join(subpath))
        {
            return Some(resolution);
        }
    } else if let Some(resolution) =
        resolve_declaration_or_runtime_candidate(&package_dir.join("index"))
    {
        return Some(resolution);
    }

    None
}

fn read_package_json(
    pkg_json_path: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<serde_json::Value> {
    if let Some(cached) = cache.package_json_cache.get(pkg_json_path) {
        return cached.clone();
    }

    let parsed = std::fs::read_to_string(pkg_json_path)
        .ok()
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok());
    cache
        .package_json_cache
        .insert(pkg_json_path.to_path_buf(), parsed.clone());
    parsed
}

fn resolve_declaration_or_runtime_candidate(path: &Path) -> Option<PackageEntrypointResolution> {
    if let Some(path) = resolve_declaration_candidate(path) {
        return Some(PackageEntrypointResolution {
            path,
            kind: PackageEntrypointKind::Declaration,
        });
    }

    resolve_runtime_only_candidate(path)
}

fn resolve_runtime_only_candidate(path: &Path) -> Option<PackageEntrypointResolution> {
    for candidate in runtime_javascript_candidates(path.to_path_buf()) {
        if candidate.exists() && candidate.is_file() {
            return Some(PackageEntrypointResolution {
                path: candidate,
                kind: PackageEntrypointKind::RuntimeOnly,
            });
        }
    }

    None
}

fn types_package_name(package_name: &str) -> String {
    package_name
        .strip_prefix('@')
        .map(|name| name.replace('/', "__"))
        .unwrap_or_else(|| package_name.to_string())
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
                if is_external_specifier(&import.module_specifier)
                    && !queued_specifiers.contains(&import.module_specifier)
                    && let Some((package_name, subpath)) =
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
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
                module_specifier: Some(module_specifier),
                ..
            })
            | ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
                module_specifier,
                ..
            }) => {
                if is_external_specifier(&module_specifier)
                    && !queued_specifiers.contains(&module_specifier)
                    && let Some((package_name, subpath)) =
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
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        assert_eq!(
            resolve_exports_types(&exports, "./feature-nested"),
            Some("./dist/feature.d.ts".to_string())
        );
        assert_eq!(resolve_exports_types(&exports, "./wild/*"), None);
        assert_eq!(resolve_exports_types(&exports, "./wild/feature"), None);
        assert_eq!(resolve_exports_types(&exports, "./missing"), None);
    }

    #[test]
    fn test_resolve_exports_types_nested_conditions() {
        let exports = serde_json::json!({
            ".": {
                "import": {
                    "types": "./dist/index.d.ts"
                }
            },
            "./subpath": {
                "default": {
                    "types": "./dist/subpath.d.ts"
                }
            }
        });

        assert_eq!(
            resolve_exports_types(&exports, "."),
            Some("./dist/index.d.ts".to_string())
        );
        assert_eq!(
            resolve_exports_types(&exports, "./subpath"),
            Some("./dist/subpath.d.ts".to_string())
        );
    }

    #[test]
    fn test_resolve_exports_entrypoint_prefers_types_then_runtime() {
        let exports = serde_json::json!({
            ".": {
                "import": "./dist/index.js",
                "types": "./dist/index.d.ts"
            },
            "./feature": {
                "default": {
                    "require": "./dist/feature.cjs",
                    "import": "./dist/feature.js"
                }
            }
        });

        assert_eq!(
            resolve_exports_entrypoint(&exports, "."),
            Some("./dist/index.d.ts".to_string())
        );
        assert_eq!(
            resolve_exports_entrypoint(&exports, "./feature"),
            Some("./dist/feature.js".to_string())
        );
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

    #[test]
    fn test_types_package_name_scoped_package() {
        assert_eq!(types_package_name("@scope/pkg"), "scope__pkg");
        assert_eq!(types_package_name("pkg"), "pkg");
    }

    #[test]
    fn test_read_package_json_is_cached() {
        let root =
            std::env::temp_dir().join(format!("package-declarations-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let pkg_json = root.join("package.json");
        fs::write(&pkg_json, r#"{ "types": "./index.d.ts" }"#).unwrap();

        let mut cache = PackageDeclarationResolverCache::default();
        let first = read_package_json(&pkg_json, &mut cache);
        let second = read_package_json(&pkg_json, &mut cache);

        assert_eq!(first, second);
        assert!(cache.package_json_cache.contains_key(&pkg_json));
    }
}
