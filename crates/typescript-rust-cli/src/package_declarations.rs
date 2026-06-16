use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use typescript_rust_checker::SourceFileInput;
use typescript_rust_config::canonicalize_if_exists_string;
use typescript_rust_syntax::{
    ParsedExportDeclaration, ParsedStatement, ReferenceTypeDirective, TextSpan,
    extract_reference_path_directives, extract_reference_type_directives, parse_source,
};

use crate::package_resolution::{
    ResolverOptions, select_export_target, select_import_target, types_versions_candidates,
};

pub struct PackageDeclarationRequest {
    pub specifier: String,
    pub package_name: String,
    pub subpath: Option<String>,
    pub importer_dir: PathBuf,
    pub importer_file: PathBuf,
    /// `#alias` specifier resolved through the importer's own `imports` field.
    pub is_imports: bool,
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
    is_imports: bool,
    importer_is_esm: bool,
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
pub fn resolve_package_declaration_entrypoints(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
) -> HashMap<String, String> {
    let mut cache = PackageDeclarationResolverCache::default();
    resolve_package_declaration_entrypoints_with_cache(
        inputs,
        sources,
        root_dir,
        &ResolverOptions::default(),
        &mut cache,
    )
}

pub(crate) fn resolve_package_declaration_entrypoints_with_cache(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    opts: &ResolverOptions,
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
            opts,
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

        let importer_is_esm = importer_is_esm(&req.importer_file, opts, cache);
        let cache_key = PackageEntrypointCacheKey {
            importer_dir: canonicalize_if_exists_string(&req.importer_dir),
            package_name: req.package_name.clone(),
            subpath: req.subpath.clone(),
            is_imports: req.is_imports,
            importer_is_esm,
        };

        let resolution = if let Some(cached) = cache.entrypoint_cache.get(&cache_key) {
            cached.clone()
        } else {
            let resolved = resolve_package_entrypoint(&req, opts, importer_is_esm, cache, root_dir);
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
                        opts,
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

/// Outcome of resolving the project's configured type packages.
pub(crate) struct TypePackageResolution {
    /// Type-package names actually included, in load order. Passed to the checker
    /// as `CheckerOptions.types` so node-specific builtins and the `@types`
    /// ambient-global gate fire for them.
    pub effective_type_names: Vec<String>,
    /// Explicitly-listed `types` entries that could not be resolved. The caller
    /// emits a TS2688 for each; wildcard-discovered packages never appear here.
    pub missing: Vec<String>,
}

/// Resolve the project's type packages and load them as dependency declarations.
///
/// Mirrors TypeScript 6.0's `getAutomaticTypeDirectiveNames`:
///
/// * `types` absent (`None`) or `types: []` → include nothing. TypeScript 6.0
///   removed the implicit inclusion of every visible `@types` package; automatic
///   discovery is now opt-in via the `"*"` wildcard below.
/// * `types` containing `"*"` → expand the wildcard to every package found under
///   the effective type roots (`typeRoots` if set, otherwise the ancestor
///   `node_modules/@types` chain), skipping dot-prefixed and "not needed"
///   packages. Other literal entries are preserved; the list is deduped in order.
/// * each resulting name resolves nearest-root-first; scoped names are mangled
///   (`@scope/pkg` -> `scope__pkg`) only under `@types` roots, matching
///   `getCandidateFromTypeRoot`.
///
/// Entrypoint resolution stays narrow: `types` / `typings` / exact
/// `exports["."].types` / `index.d.ts`.
pub(crate) fn resolve_type_packages(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    types: Option<&[String]>,
    type_roots: &[PathBuf],
    cache: &mut PackageDeclarationResolverCache,
) -> TypePackageResolution {
    let mut resolution = TypePackageResolution {
        effective_type_names: Vec::new(),
        missing: Vec::new(),
    };

    // `types` absent or empty: nothing is auto-included (TypeScript 6.0).
    let Some(configured) = types else {
        return resolution;
    };
    if configured.is_empty() {
        return resolution;
    }

    let roots = effective_type_roots(root_dir, type_roots);

    // Expand `"*"` entries to the packages discovered under the type roots.
    let wildcard_matches = if configured.iter().any(|entry| entry == "*") {
        discover_wildcard_type_names(&roots, cache)
    } else {
        Vec::new()
    };

    // Flatten the directive list in order, deduping by name. `explicit` entries
    // emit TS2688 when unresolved; wildcard matches never do (they came from
    // directories that exist).
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut directives: Vec<(String, bool)> = Vec::new();
    for entry in configured {
        if entry == "*" {
            for name in &wildcard_matches {
                if seen_names.insert(name.clone()) {
                    directives.push((name.clone(), false));
                }
            }
        } else if seen_names.insert(entry.clone()) {
            directives.push((entry.clone(), true));
        }
    }

    let mut known_file_names: HashSet<String> = inputs
        .iter()
        .map(|input| canonicalize_if_exists_string(Path::new(&input.file_name)))
        .collect();

    for (name, explicit) in directives {
        match resolve_type_directive_in_roots(&name, &roots, cache) {
            Some(path) => {
                load_type_package_file(&path, inputs, sources, &mut known_file_names);
                resolution.effective_type_names.push(name);
            }
            None => {
                if explicit {
                    resolution.missing.push(name);
                }
            }
        }
    }

    resolution
}

/// A `/// <reference types="..." />` site that could not be resolved to a type
/// package. The caller emits a TS2688 located at `value_span` in `file_name`.
pub(crate) struct MissingReferenceTypeDirective {
    pub file_name: String,
    pub type_name: String,
    pub value_span: TextSpan,
    /// Whether the referencing file is a declaration (`.d.ts`) file. tsc gates the
    /// TS2688 from such a site behind `skipLibCheck`, like any other `.d.ts`
    /// diagnostic.
    pub from_declaration_file: bool,
}

/// Outcome of resolving every `/// <reference types>` directive reachable from the
/// program.
pub(crate) struct ReferenceTypeDirectiveResolution {
    pub effective_type_names: Vec<String>,
    pub missing: Vec<MissingReferenceTypeDirective>,
}

/// Resolves explicit `/// <reference types="..." />` directives against the same
/// type roots and entrypoint logic as `compilerOptions.types`. The resolver is
/// stateful so it can be re-run as the file set grows during import-graph and
/// package-declaration expansion: each call scans only files not seen before and
/// follows references recursively (a loaded type package's own directives).
pub(crate) struct ReferenceTypeDirectiveResolver {
    roots: Vec<PathBuf>,
    scanned_files: HashSet<String>,
    resolution_cache: HashMap<String, Option<PathBuf>>,
    effective_type_names: Vec<String>,
    seen_effective: HashSet<String>,
    missing: Vec<MissingReferenceTypeDirective>,
}

impl ReferenceTypeDirectiveResolver {
    pub fn new(root_dir: &Path, type_roots: &[PathBuf]) -> Self {
        Self {
            roots: effective_type_roots(root_dir, type_roots),
            scanned_files: HashSet::new(),
            resolution_cache: HashMap::new(),
            effective_type_names: Vec::new(),
            seen_effective: HashSet::new(),
            missing: Vec::new(),
        }
    }

    /// Scan every not-yet-scanned source for reference-type directives, loading
    /// each resolved type package into `inputs`/`sources`. Loading appends files,
    /// which are scanned in the same call, so recursive references converge here.
    pub fn scan_and_resolve(
        &mut self,
        inputs: &mut Vec<SourceFileInput>,
        sources: &mut Vec<(PathBuf, String, String)>,
        cache: &mut PackageDeclarationResolverCache,
    ) {
        loop {
            let pending: Vec<(String, String)> = sources
                .iter()
                .filter(|(_, file_name, _)| !self.scanned_files.contains(file_name))
                .map(|(_, file_name, source_text)| (file_name.clone(), source_text.clone()))
                .collect();

            if pending.is_empty() {
                break;
            }

            let mut known_file_names: HashSet<String> = inputs
                .iter()
                .map(|input| canonicalize_if_exists_string(Path::new(&input.file_name)))
                .collect();

            for (file_name, source_text) in pending {
                self.scanned_files.insert(file_name.clone());

                // `/// <reference path="..." />` pulls in a sibling declaration
                // file relative to the referencing file. This is how a type
                // package such as `@types/node` assembles its full surface
                // (`index.d.ts` references `globals.d.ts`, `buffer.d.ts`, ...),
                // where its ambient globals (`process`, `Buffer`) are declared.
                for path_value in extract_reference_path_directives(&source_text) {
                    load_reference_path_file(
                        &file_name,
                        &path_value,
                        inputs,
                        sources,
                        &mut known_file_names,
                    );
                }

                let directives = extract_reference_type_directives(&source_text);
                if directives.is_empty() {
                    continue;
                }

                let from_declaration_file = is_declaration_file_path_str(&file_name);
                for directive in directives {
                    self.resolve_directive(
                        directive,
                        &file_name,
                        from_declaration_file,
                        inputs,
                        sources,
                        &mut known_file_names,
                        cache,
                    );
                }
            }
        }
    }

    fn resolve_directive(
        &mut self,
        directive: ReferenceTypeDirective,
        file_name: &str,
        from_declaration_file: bool,
        inputs: &mut Vec<SourceFileInput>,
        sources: &mut Vec<(PathBuf, String, String)>,
        known_file_names: &mut HashSet<String>,
        cache: &mut PackageDeclarationResolverCache,
    ) {
        let name = directive.value;
        let resolved = match self.resolution_cache.get(&name) {
            Some(cached) => cached.clone(),
            None => {
                let resolved = resolve_type_directive_in_roots(&name, &self.roots, cache);
                self.resolution_cache.insert(name.clone(), resolved.clone());
                resolved
            }
        };

        match resolved {
            Some(path) => {
                load_type_package_file(&path, inputs, sources, known_file_names);
                if self.seen_effective.insert(name.clone()) {
                    self.effective_type_names.push(name);
                }
            }
            None => {
                self.missing.push(MissingReferenceTypeDirective {
                    file_name: file_name.to_string(),
                    type_name: name,
                    value_span: directive.value_span,
                    from_declaration_file,
                });
            }
        }
    }

    pub fn into_resolution(self) -> ReferenceTypeDirectiveResolution {
        ReferenceTypeDirectiveResolution {
            effective_type_names: self.effective_type_names,
            missing: self.missing,
        }
    }
}

/// The effective type roots for type-directive resolution, nearest first.
/// Mirrors `getEffectiveTypeRoots`: explicit `typeRoots` win outright; otherwise
/// every ancestor `node_modules/@types` directory (existence checked lazily when
/// scanning or resolving).
fn effective_type_roots(root_dir: &Path, type_roots: &[PathBuf]) -> Vec<PathBuf> {
    if !type_roots.is_empty() {
        return type_roots.to_vec();
    }

    let mut roots = Vec::new();
    let mut current_dir = root_dir.to_path_buf();
    loop {
        roots.push(current_dir.join("node_modules").join("@types"));
        let Some(parent) = current_dir.parent() else {
            break;
        };
        current_dir = parent.to_path_buf();
    }
    roots
}

/// Whether a type root uses the DefinitelyTyped `@types` mangling convention
/// (scoped `@scope/pkg` stored on disk as `scope__pkg`).
fn is_at_types_root(root: &Path) -> bool {
    root.ends_with(Path::new("node_modules").join("@types"))
}

/// Discover the package names contributed by a `"*"` wildcard, scanning each
/// effective type root once. Skips dot-prefixed directories and "not needed"
/// packages (`package.json` with `"typings": null`). Names are the raw directory
/// base names (the mangled form for scoped `@types` packages), matching
/// TypeScript's `getAutomaticTypeDirectiveNames`.
fn discover_wildcard_type_names(
    roots: &[PathBuf],
    cache: &mut PackageDeclarationResolverCache,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };

        let mut dir_names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect();
        dir_names.sort();

        for dir_name in dir_names {
            if is_not_needed_types_package(&root.join(&dir_name), cache) {
                continue;
            }
            if seen.insert(dir_name.clone()) {
                names.push(dir_name);
            }
        }
    }

    names
}

/// A "not needed" stub: `package.json` with `"typings": null`, used by
/// DefinitelyTyped to mark packages that ship their own types. Excluded from
/// wildcard discovery, matching TypeScript.
fn is_not_needed_types_package(
    pkg_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> bool {
    let pkg_json_path = pkg_dir.join("package.json");
    if !pkg_json_path.is_file() {
        return false;
    }
    match read_package_json(&pkg_json_path, cache) {
        Some(json) => matches!(json.get("typings"), Some(serde_json::Value::Null)),
        None => false,
    }
}

/// Resolve a type-directive name to its entrypoint, trying each root in order
/// (nearest first) and returning the first hit. Scoped names are mangled only
/// under `@types` roots; custom `typeRoots` use the name verbatim.
fn resolve_type_directive_in_roots(
    name: &str,
    roots: &[PathBuf],
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PathBuf> {
    for root in roots {
        let lookup = if is_at_types_root(root) {
            types_package_name(name)
        } else {
            name.to_string()
        };
        if let Some(path) = resolve_at_types_package_entrypoint(&root.join(&lookup), cache) {
            return Some(path);
        }
    }
    None
}

/// Add a resolved type-package declaration file to the project file set unless it
/// is already present.
/// Load the file targeted by a `/// <reference path="..." />` directive,
/// resolved relative to the referencing file's directory. The literal target is
/// tried first (the directive normally names a `.d.ts` file outright); otherwise
/// the usual declaration-candidate extensions are attempted.
fn load_reference_path_file(
    referencing_file: &str,
    path_value: &str,
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    known_file_names: &mut HashSet<String>,
) {
    let Some(base_dir) = Path::new(referencing_file).parent() else {
        return;
    };

    let candidate = base_dir.join(path_value);
    let resolved = if candidate.is_file() {
        Some(candidate)
    } else {
        resolve_declaration_candidate(&candidate)
    };

    if let Some(path) = resolved {
        load_type_package_file(&path, inputs, sources, known_file_names);
    }
}

fn load_type_package_file(
    path: &Path,
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    known_file_names: &mut HashSet<String>,
) {
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let normalized_file_name = canonicalize_if_exists_string(&canonical_path);
    if !known_file_names.insert(normalized_file_name.clone()) {
        return;
    }

    let Ok(source_text) = std::fs::read_to_string(&canonical_path) else {
        return;
    };

    inputs.push(SourceFileInput {
        file_name: normalized_file_name.clone(),
        source_text: source_text.clone(),
    });
    sources.push((canonical_path, normalized_file_name, source_text));
}

fn resolve_at_types_package_entrypoint(
    pkg_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PathBuf> {
    let pkg_json_path = pkg_dir.join("package.json");
    if pkg_json_path.is_file() {
        if let Some(json) = read_package_json(&pkg_json_path, cache) {
            if let Some(types) = json.get("types").and_then(|t| t.as_str()) {
                if let Some(path) = resolve_declaration_candidate(&pkg_dir.join(types)) {
                    return Some(path);
                }
            }

            if let Some(typings) = json.get("typings").and_then(|t| t.as_str()) {
                if let Some(path) = resolve_declaration_candidate(&pkg_dir.join(typings)) {
                    return Some(path);
                }
            }

            if let Some(exports) = json.get("exports") {
                let conditions = ResolverOptions::default().active_conditions(true);
                if let Some(types_path) = select_export_target(exports, ".", &conditions) {
                    if let Some(path) = resolve_declaration_candidate(&pkg_dir.join(types_path)) {
                        return Some(path);
                    }
                }
            }
        }
    }

    resolve_declaration_candidate(&pkg_dir.join("index"))
}

fn resolve_package_entrypoint(
    req: &PackageDeclarationRequest,
    opts: &ResolverOptions,
    importer_is_esm: bool,
    cache: &mut PackageDeclarationResolverCache,
    root_dir: &Path,
) -> Option<PackageEntrypointResolution> {
    // `#alias` imports resolve against the importer's own enclosing package.
    if req.is_imports {
        return resolve_imports_entrypoint(req, opts, importer_is_esm, cache);
    }

    // Package self-name imports: an enclosing package whose `name` matches takes
    // priority over external `node_modules` lookup, mirroring TypeScript.
    if let Some(resolution) = resolve_self_name_entrypoint(req, opts, importer_is_esm, cache) {
        return Some(resolution);
    }

    let mut current_dir = req.importer_dir.clone();
    let mut runtime_fallback = None;

    loop {
        let pkg_dir = current_dir.join("node_modules").join(&req.package_name);

        if let Some(resolution) =
            resolve_package_entrypoint_in_directory(req, &pkg_dir, opts, importer_is_esm, cache)
        {
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

/// Resolve a `#alias` import against the nearest enclosing `package.json` with an
/// `imports` field. Blocked or unresolved aliases yield `None` (→ TS2307).
fn resolve_imports_entrypoint(
    req: &PackageDeclarationRequest,
    opts: &ResolverOptions,
    importer_is_esm: bool,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PackageEntrypointResolution> {
    if !opts.resolve_imports {
        return None;
    }

    let (pkg_dir, json) = nearest_package_json(&req.importer_dir, cache)?;
    let imports = json.get("imports")?;
    let conditions = opts.active_conditions(importer_is_esm);
    let target = select_import_target(imports, &req.specifier, &conditions)?;
    resolve_target_in_package(&pkg_dir, &target)
}

/// Resolve a bare package import through an enclosing package whose `name`
/// matches `req.package_name` (package self-reference).
fn resolve_self_name_entrypoint(
    req: &PackageDeclarationRequest,
    opts: &ResolverOptions,
    importer_is_esm: bool,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PackageEntrypointResolution> {
    if !opts.resolve_exports {
        return None;
    }

    let (pkg_dir, json) = nearest_package_json(&req.importer_dir, cache)?;
    let name = json.get("name").and_then(|n| n.as_str())?;
    if name != req.package_name {
        return None;
    }
    // Self-name only works through the package's own `exports` map.
    let exports = json.get("exports")?;
    let subpath_key = subpath_key(req);
    let conditions = opts.active_conditions(importer_is_esm);
    let target = select_export_target(exports, &subpath_key, &conditions)?;
    resolve_target_in_package(&pkg_dir, &target)
}

fn resolve_package_entrypoint_in_directory(
    req: &PackageDeclarationRequest,
    pkg_dir: &Path,
    opts: &ResolverOptions,
    importer_is_esm: bool,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PackageEntrypointResolution> {
    let pkg_json_path = pkg_dir.join("package.json");
    let json = if pkg_json_path.is_file() {
        read_package_json(&pkg_json_path, cache)
    } else {
        None
    };

    if let Some(json) = &json {
        // Modern `exports` is authoritative: when present (and honored), a
        // non-matching subpath is blocked rather than falling back to file
        // probing, matching TypeScript's node16/nodenext/bundler behavior.
        if opts.resolve_exports {
            if let Some(exports) = json.get("exports") {
                let subpath_key = subpath_key(req);
                let conditions = opts.active_conditions(importer_is_esm);
                return select_export_target(exports, &subpath_key, &conditions)
                    .and_then(|target| resolve_target_in_package(pkg_dir, &target));
            }
        }

        return resolve_legacy_entrypoint_in_directory(req, pkg_dir, json);
    }

    // No `package.json`: legacy file probing only.
    resolve_legacy_file_probe(req, pkg_dir)
}

/// Legacy (`node10`-style) entrypoint resolution for a package without a usable
/// `exports` field: `typesVersions`, then `types`/`typings`/`module`/`main`,
/// then conventional `index` locations.
fn resolve_legacy_entrypoint_in_directory(
    req: &PackageDeclarationRequest,
    pkg_dir: &Path,
    json: &serde_json::Value,
) -> Option<PackageEntrypointResolution> {
    let mut runtime_fallback = None;
    let types_versions = json.get("typesVersions");

    macro_rules! try_candidate {
        ($path:expr) => {
            if let Some(resolution) = resolve_declaration_or_runtime_candidate($path) {
                match resolution.kind {
                    PackageEntrypointKind::Declaration => return Some(resolution),
                    PackageEntrypointKind::RuntimeOnly => {
                        if runtime_fallback.is_none() {
                            runtime_fallback = Some(resolution);
                        }
                    }
                }
            }
        };
    }

    if let Some(subpath) = &req.subpath {
        // `typesVersions` rewrites the subpath before file probing.
        if let Some(types_versions) = types_versions {
            for target in types_versions_candidates(types_versions, subpath) {
                if let Some(resolution) = resolve_target_in_package(pkg_dir, &target) {
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

        try_candidate!(&pkg_dir.join(subpath));
        try_candidate!(&pkg_dir.join(subpath).join("index"));
    } else {
        // Root: apply `typesVersions` to the declared types field, then fall back
        // to `index`.
        if let Some(types_versions) = types_versions {
            for base in root_types_version_bases(json) {
                for target in types_versions_candidates(types_versions, &base) {
                    if let Some(resolution) = resolve_target_in_package(pkg_dir, &target) {
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

        for field in ["types", "typings", "module", "main"] {
            if let Some(value) = json.get(field).and_then(|t| t.as_str()) {
                try_candidate!(&pkg_dir.join(value));
            }
        }

        for candidate in [
            "dist/types/index",
            "types/index",
            "typings/index",
            "dist/esm/index",
            "dist/index",
        ] {
            try_candidate!(&pkg_dir.join(candidate));
        }
    }

    if let Some(resolution) = resolve_legacy_file_probe(req, pkg_dir) {
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

/// Bare `subpath`/`index` probing for a package directory with no usable
/// `package.json` metadata.
fn resolve_legacy_file_probe(
    req: &PackageDeclarationRequest,
    pkg_dir: &Path,
) -> Option<PackageEntrypointResolution> {
    if let Some(subpath) = &req.subpath {
        if let Some(resolution) = resolve_declaration_or_runtime_candidate(&pkg_dir.join(subpath)) {
            return Some(resolution);
        }
        resolve_declaration_or_runtime_candidate(&pkg_dir.join(subpath).join("index"))
    } else {
        resolve_declaration_or_runtime_candidate(&pkg_dir.join("index"))
    }
}

/// The `exports`/self-name subpath key for a request: `"."` for the package root,
/// `"./<subpath>"` otherwise.
fn subpath_key(req: &PackageDeclarationRequest) -> String {
    match &req.subpath {
        Some(subpath) => format!("./{}", subpath),
        None => ".".to_string(),
    }
}

/// Package-relative base paths a root `typesVersions` mapping is applied to: the
/// declared `types`/`typings` field (without the leading `./`) and the
/// conventional `index.d.ts`.
fn root_types_version_bases(json: &serde_json::Value) -> Vec<String> {
    let mut bases = Vec::new();
    for field in ["types", "typings"] {
        if let Some(value) = json.get(field).and_then(|t| t.as_str()) {
            bases.push(value.trim_start_matches("./").to_string());
        }
    }
    bases.push("index.d.ts".to_string());
    bases
}

/// Join an `exports`/`imports`/`typesVersions` target against the package root,
/// rejecting paths that escape the package, then probe declaration variants.
fn resolve_target_in_package(pkg_dir: &Path, target: &str) -> Option<PackageEntrypointResolution> {
    let relative = target.trim_start_matches("./");
    let joined = pkg_dir.join(relative);
    if !path_is_within(pkg_dir, &joined) {
        return None;
    }
    resolve_declaration_or_runtime_candidate(&joined)
}

/// Whether `candidate` stays within `base` after resolving `..` segments
/// lexically (no filesystem access). Guards against `exports` targets escaping
/// the package root.
fn path_is_within(base: &Path, candidate: &Path) -> bool {
    use std::path::Component;
    let mut depth: i32 = 0;
    for component in candidate
        .strip_prefix(base)
        .unwrap_or(candidate)
        .components()
    {
        match component {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::CurDir => {}
            _ => depth += 1,
        }
    }
    true
}

/// Find the nearest enclosing `package.json` walking up from `start_dir`,
/// returning the package directory and its parsed contents.
fn nearest_package_json(
    start_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<(PathBuf, serde_json::Value)> {
    let mut current = Some(start_dir.to_path_buf());
    while let Some(dir) = current {
        // Never cross a `node_modules` boundary upward into an unrelated package.
        let pkg_json_path = dir.join("package.json");
        if pkg_json_path.is_file() {
            if let Some(json) = read_package_json(&pkg_json_path, cache) {
                return Some((dir, json));
            }
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Whether the importing file is treated as ESM for condition selection. Bundler
/// always behaves as ESM; node16/nodenext consult the file extension and the
/// nearest `package.json` `"type"`.
fn importer_is_esm(
    importer_file: &Path,
    opts: &ResolverOptions,
    cache: &mut PackageDeclarationResolverCache,
) -> bool {
    use typescript_rust_config::ModuleResolutionKind;
    if opts.module_resolution == ModuleResolutionKind::Bundler {
        return true;
    }

    let lower = importer_file.to_string_lossy().to_ascii_lowercase();
    if lower.ends_with(".mts") || lower.ends_with(".mjs") || lower.ends_with(".d.mts") {
        return true;
    }
    if lower.ends_with(".cts") || lower.ends_with(".cjs") || lower.ends_with(".d.cts") {
        return false;
    }

    let start = importer_file.parent().unwrap_or(importer_file);
    match nearest_package_json(start, cache) {
        Some((_, json)) => json.get("type").and_then(|t| t.as_str()) == Some("module"),
        None => false,
    }
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
    opts: &ResolverOptions,
    packages_to_resolve: &mut VecDeque<PackageDeclarationRequest>,
    queued_specifiers: &mut HashSet<String>,
) {
    let importer_file = PathBuf::from(file_name);
    let parsed = parse_source(source_text, file_name);
    for statement in parsed.statements {
        let specifier = match statement {
            ParsedStatement::ImportDeclaration(import) => Some(import.module_specifier),
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
                module_specifier: Some(module_specifier),
                ..
            })
            | ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
                module_specifier,
                ..
            }) => Some(module_specifier),
            _ => None,
        };

        let Some(specifier) = specifier else {
            continue;
        };

        queue_specifier(
            &specifier,
            importer_dir,
            &importer_file,
            opts,
            packages_to_resolve,
            queued_specifiers,
        );
    }
}

/// Queue a module specifier for package resolution. Handles `#alias` imports and
/// bare/scoped package specifiers; relative specifiers are ignored (handled by
/// the import-graph expander).
fn queue_specifier(
    specifier: &str,
    importer_dir: &Path,
    importer_file: &Path,
    opts: &ResolverOptions,
    packages_to_resolve: &mut VecDeque<PackageDeclarationRequest>,
    queued_specifiers: &mut HashSet<String>,
) {
    if queued_specifiers.contains(specifier) {
        return;
    }

    if let Some(rest) = specifier.strip_prefix('#') {
        // `#` alone or `#/...` is not a valid imports key.
        if rest.is_empty() || !opts.resolve_imports {
            return;
        }
        queued_specifiers.insert(specifier.to_string());
        packages_to_resolve.push_back(PackageDeclarationRequest {
            specifier: specifier.to_string(),
            package_name: specifier.to_string(),
            subpath: None,
            importer_dir: importer_dir.to_path_buf(),
            importer_file: importer_file.to_path_buf(),
            is_imports: true,
        });
        return;
    }

    if !is_external_specifier(specifier) {
        return;
    }

    if let Some((package_name, subpath)) = parse_package_specifier(specifier) {
        queued_specifiers.insert(specifier.to_string());
        packages_to_resolve.push_back(PackageDeclarationRequest {
            specifier: specifier.to_string(),
            package_name,
            subpath,
            importer_dir: importer_dir.to_path_buf(),
            importer_file: importer_file.to_path_buf(),
            is_imports: false,
        });
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
    fn test_path_is_within_rejects_escape() {
        let base = Path::new("/pkg");
        assert!(path_is_within(base, &base.join("dist/index.d.ts")));
        assert!(path_is_within(base, &base.join("a/../b/index.d.ts")));
        assert!(!path_is_within(base, &base.join("../outside.d.ts")));
        assert!(!path_is_within(base, &base.join("a/../../escape.d.ts")));
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
