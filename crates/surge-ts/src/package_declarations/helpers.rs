//! Private resolution helpers for [`super`]: package.json parsing, entrypoint
//! and type-root probing, declaration/runtime candidate selection, and path
//! classification. All state flows through arguments; no public surface.

use super::*;

/// The effective type roots for type-directive resolution, nearest first.
/// Mirrors `getEffectiveTypeRoots`: explicit `typeRoots` win outright; otherwise
/// every ancestor `node_modules/@types` directory (existence checked lazily when
/// scanning or resolving).
pub(super) fn effective_type_roots(root_dir: &Path, type_roots: &[PathBuf]) -> Vec<PathBuf> {
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
pub(super) fn is_at_types_root(root: &Path) -> bool {
    root.ends_with(Path::new("node_modules").join("@types"))
}

/// Discover the package names contributed by a `"*"` wildcard, scanning each
/// effective type root once. Skips dot-prefixed directories and "not needed"
/// packages (`package.json` with `"typings": null`). Names are the raw directory
/// base names (the mangled form for scoped `@types` packages), matching
/// TypeScript's `getAutomaticTypeDirectiveNames`.
pub(super) fn discover_wildcard_type_names(
    roots: &[PathBuf],
    cache: &mut PackageDeclarationResolverCache,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for root in roots {
        let list_start = std::time::Instant::now();
        let entries = std::fs::read_dir(root);
        crate::io_stats::record_read_dir(list_start.elapsed());
        let Ok(entries) = entries else {
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
pub(super) fn is_not_needed_types_package(
    pkg_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> bool {
    let pkg_json_path = pkg_dir.join("package.json");
    if !crate::probe::is_existing_file(&pkg_json_path) {
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
pub(super) fn resolve_type_directive_in_roots(
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
pub(super) fn load_reference_path_file(
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

pub(super) fn load_type_package_file(
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

    let read_start = std::time::Instant::now();
    let Ok(source_text) = std::fs::read_to_string(&canonical_path) else {
        return;
    };
    crate::io_stats::record_expansion_read(source_text.len(), read_start.elapsed());

    inputs.push(SourceFileInput {
        file_name: normalized_file_name.clone(),
        source_text: source_text.clone(),
    });
    sources.push((canonical_path, normalized_file_name, source_text));
}

pub(super) fn resolve_at_types_package_entrypoint(
    pkg_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PathBuf> {
    let pkg_json_path = pkg_dir.join("package.json");
    if crate::probe::is_existing_file(&pkg_json_path) {
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

pub(super) fn resolve_package_entrypoint(
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
pub(super) fn resolve_imports_entrypoint(
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
    let targets = select_import_targets(imports, &req.specifier, &conditions);
    resolve_first_target_in_package(&pkg_dir, &targets)
}

/// Resolve a bare package import through an enclosing package whose `name`
/// matches `req.package_name` (package self-reference).
pub(super) fn resolve_self_name_entrypoint(
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
    let targets = select_export_targets(exports, &subpath_key, &conditions);
    resolve_first_target_in_package(&pkg_dir, &targets)
}

pub(super) fn resolve_package_entrypoint_in_directory(
    req: &PackageDeclarationRequest,
    pkg_dir: &Path,
    opts: &ResolverOptions,
    importer_is_esm: bool,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<PackageEntrypointResolution> {
    let pkg_json_path = pkg_dir.join("package.json");
    let json = if crate::probe::is_existing_file(&pkg_json_path) {
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
                let targets = select_export_targets(exports, &subpath_key, &conditions);
                return resolve_first_target_in_package(pkg_dir, &targets);
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
pub(super) fn resolve_legacy_entrypoint_in_directory(
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
pub(super) fn resolve_legacy_file_probe(
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
pub(super) fn subpath_key(req: &PackageDeclarationRequest) -> String {
    match &req.subpath {
        Some(subpath) => format!("./{}", subpath),
        None => ".".to_string(),
    }
}

/// Package-relative base paths a root `typesVersions` mapping is applied to: the
/// declared `types`/`typings` field (without the leading `./`) and the
/// conventional `index.d.ts`.
pub(super) fn root_types_version_bases(json: &serde_json::Value) -> Vec<String> {
    let mut bases = Vec::new();
    for field in ["types", "typings"] {
        if let Some(value) = json.get(field).and_then(|t| t.as_str()) {
            bases.push(value.trim_start_matches("./").to_string());
        }
    }
    bases.push("index.d.ts".to_string());
    bases
}

/// Probe each candidate target in priority order, preferring the first
/// declaration-kind resolution; a runtime-only hit is kept as a fallback so a
/// lower-priority condition can still supply declarations. This mirrors tsc,
/// which falls through to the next matching `exports`/`imports` condition when
/// the selected target does not resolve to a type-providing file.
pub(super) fn resolve_first_target_in_package(
    pkg_dir: &Path,
    targets: &[String],
) -> Option<PackageEntrypointResolution> {
    let mut runtime_fallback = None;
    for target in targets {
        if let Some(resolution) = resolve_target_in_package(pkg_dir, target) {
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
    runtime_fallback
}

/// Join an `exports`/`imports`/`typesVersions` target against the package root,
/// rejecting paths that escape the package, then probe declaration variants.
pub(super) fn resolve_target_in_package(
    pkg_dir: &Path,
    target: &str,
) -> Option<PackageEntrypointResolution> {
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
pub(super) fn path_is_within(base: &Path, candidate: &Path) -> bool {
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
pub(super) fn nearest_package_json(
    start_dir: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<(PathBuf, std::sync::Arc<serde_json::Value>)> {
    let mut current = Some(start_dir.to_path_buf());
    while let Some(dir) = current {
        // Never cross a `node_modules` boundary upward into an unrelated package.
        let pkg_json_path = dir.join("package.json");
        if crate::probe::is_existing_file(&pkg_json_path) {
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
pub(super) fn importer_is_esm(
    importer_file: &Path,
    opts: &ResolverOptions,
    cache: &mut PackageDeclarationResolverCache,
) -> bool {
    use surge_ts_config::ModuleResolutionKind;
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

pub(super) fn resolve_at_types_fallback_in_directory(
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

pub(super) fn resolve_types_package_directory(
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

// Hits vastly outnumber misses (every candidate spelling of a package re-reads
// its `package.json`), and a parsed manifest with a large `exports` map is
// expensive to deep-clone, so the cache hands out shared handles.
pub(super) fn read_package_json(
    pkg_json_path: &Path,
    cache: &mut PackageDeclarationResolverCache,
) -> Option<std::sync::Arc<serde_json::Value>> {
    if let Some(cached) = cache.package_json_cache.get(pkg_json_path) {
        return cached.clone();
    }

    crate::io_stats::record_package_json_read();
    let parsed = std::fs::read_to_string(pkg_json_path)
        .ok()
        .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
        .map(std::sync::Arc::new);
    cache
        .package_json_cache
        .insert(pkg_json_path.to_path_buf(), parsed.clone());
    parsed
}

pub(super) fn resolve_declaration_or_runtime_candidate(
    path: &Path,
) -> Option<PackageEntrypointResolution> {
    if let Some(path) = resolve_declaration_candidate(path) {
        return Some(PackageEntrypointResolution {
            path,
            kind: PackageEntrypointKind::Declaration,
        });
    }

    resolve_runtime_only_candidate(path)
}

pub(super) fn resolve_runtime_only_candidate(path: &Path) -> Option<PackageEntrypointResolution> {
    for candidate in runtime_javascript_candidates(path.to_path_buf()) {
        if crate::probe::is_existing_file(&candidate) {
            return Some(PackageEntrypointResolution {
                path: candidate,
                kind: PackageEntrypointKind::RuntimeOnly,
            });
        }
    }

    None
}

pub(super) fn types_package_name(package_name: &str) -> String {
    package_name
        .strip_prefix('@')
        .map(|name| name.replace('/', "__"))
        .unwrap_or_else(|| package_name.to_string())
}

pub(super) fn resolve_declaration_candidate(path: &Path) -> Option<PathBuf> {
    if is_declaration_file_path(path) && crate::probe::is_existing_file(path) {
        return Some(path.to_path_buf());
    }

    for candidate in declaration_candidates(path.to_path_buf()) {
        if crate::probe::is_existing_file(&candidate) {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn declaration_candidates(path: PathBuf) -> Vec<PathBuf> {
    if is_declaration_file_path(&path) {
        return vec![path];
    }

    // An explicit TypeScript-source target (a source-condition `exports` entry
    // like `"@zod/source": "./src/index.ts"`, or a `main`/`types` field naming a
    // shipped source) resolves to the source file itself, matching tsc — the
    // file is loaded and checked like any other program source.
    if is_typescript_source_file(&path) {
        return vec![path];
    }

    // A runtime-JavaScript target substitutes its own module flavor, source
    // extensions before declarations (tsc probes `.ts`/`.tsx` ahead of `.d.ts`
    // even inside `node_modules`, so a dependency shipping sources gets them).
    if is_runtime_javascript_file(&path) {
        let stem = path.with_extension("");
        return if path_ends_with_ignore_ascii_case(&path, ".mjs") {
            vec![stem.with_extension("mts"), stem.with_extension("d.mts")]
        } else if path_ends_with_ignore_ascii_case(&path, ".cjs") {
            vec![stem.with_extension("cts"), stem.with_extension("d.cts")]
        } else {
            vec![
                stem.with_extension("ts"),
                stem.with_extension("tsx"),
                stem.with_extension("d.ts"),
            ]
        };
    }

    if path.extension().is_some() {
        return Vec::new();
    }

    vec![
        path.with_extension("ts"),
        path.with_extension("tsx"),
        path.with_extension("d.ts"),
        path.with_extension("mts"),
        path.with_extension("cts"),
        path.with_extension("d.mts"),
        path.with_extension("d.cts"),
    ]
}

// Suffix checks run per candidate in the resolution hot loops; matching on
// raw bytes avoids the `to_string_lossy().to_ascii_lowercase()` double
// allocation. Lossy conversion only replaces invalid UTF-8 with U+FFFD
// (non-ASCII), so an ASCII suffix matches the lossy string iff it matches
// the raw bytes.
fn bytes_end_with_ignore_ascii_case(bytes: &[u8], suffix: &str) -> bool {
    bytes.len() >= suffix.len()
        && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

fn path_ends_with_ignore_ascii_case(path: &Path, suffix: &str) -> bool {
    bytes_end_with_ignore_ascii_case(path.as_os_str().as_encoded_bytes(), suffix)
}

pub(super) fn is_declaration_file_path_str(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes_end_with_ignore_ascii_case(bytes, ".d.ts")
        || bytes_end_with_ignore_ascii_case(bytes, ".d.mts")
        || bytes_end_with_ignore_ascii_case(bytes, ".d.cts")
}

pub(super) fn is_declaration_file_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    bytes_end_with_ignore_ascii_case(bytes, ".d.ts")
        || bytes_end_with_ignore_ascii_case(bytes, ".d.mts")
        || bytes_end_with_ignore_ascii_case(bytes, ".d.cts")
}

pub(super) fn is_runtime_javascript_file(path: &Path) -> bool {
    path_ends_with_ignore_ascii_case(path, ".js")
        || path_ends_with_ignore_ascii_case(path, ".jsx")
        || path_ends_with_ignore_ascii_case(path, ".mjs")
        || path_ends_with_ignore_ascii_case(path, ".cjs")
}

pub(super) fn is_typescript_source_file(path: &Path) -> bool {
    if is_declaration_file_path(path) {
        return false;
    }
    path_ends_with_ignore_ascii_case(path, ".ts")
        || path_ends_with_ignore_ascii_case(path, ".tsx")
        || path_ends_with_ignore_ascii_case(path, ".mts")
        || path_ends_with_ignore_ascii_case(path, ".cts")
}

pub(super) fn runtime_javascript_candidates(path: PathBuf) -> Vec<PathBuf> {
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

pub(super) fn extract_packages_from_source(
    specifiers: &[String],
    file_name: &str,
    importer_dir: &Path,
    opts: &ResolverOptions,
    packages_to_resolve: &mut VecDeque<PackageDeclarationRequest>,
    queued_specifiers: &mut HashSet<(String, String)>,
) {
    let importer_file = PathBuf::from(file_name);
    for specifier in specifiers {
        queue_specifier(
            specifier,
            importer_dir,
            &importer_file,
            opts,
            packages_to_resolve,
            queued_specifiers,
        );
    }
}

/// Whether a bare specifier is claimed by tsconfig `paths`: a pattern matches
/// and one of the mapped targets exists as a loadable TypeScript file (the
/// same candidate set and filter the import-graph expander uses). When true,
/// resolution ends at the mapped file and tsc performs no `node_modules`
/// fallback, so the package-declaration walk must be skipped.
fn resolved_by_path_mapping(specifier: &str, opts: &ResolverOptions) -> bool {
    if opts.path_mappings.is_empty() {
        return false;
    }
    let Some(base) = opts.path_mapping_base.as_deref() else {
        return false;
    };
    let Some(targets) = surge_ts_config::select_path_mapping_targets(specifier, &opts.path_mappings)
    else {
        return false;
    };
    for target in targets {
        let joined = surge_ts_config::normalize_path_string(&base.join(&target).to_string_lossy());
        for candidate in
            surge_ts_checker::lowlevel::resolution_candidates::mapped_target_candidates(&joined)
        {
            let lower = candidate.to_ascii_lowercase();
            let loadable = lower.ends_with(".ts")
                || lower.ends_with(".tsx")
                || lower.ends_with(".mts")
                || lower.ends_with(".cts");
            if loadable && crate::probe::is_existing_file(Path::new(&candidate)) {
                return true;
            }
        }
    }
    false
}

/// Queue a module specifier for package resolution. Handles `#alias` imports and
/// bare/scoped package specifiers; relative specifiers are ignored (handled by
/// the import-graph expander). Deduplication is per (importer file, specifier):
/// the same specifier resolves independently from each importer so nested
/// `node_modules` and `#imports` scopes stay isolated.
pub(super) fn queue_specifier(
    specifier: &str,
    importer_dir: &Path,
    importer_file: &Path,
    opts: &ResolverOptions,
    packages_to_resolve: &mut VecDeque<PackageDeclarationRequest>,
    queued_specifiers: &mut HashSet<(String, String)>,
) {
    let queue_key = (
        canonicalize_if_exists_string(importer_file),
        specifier.to_string(),
    );
    if queued_specifiers.contains(&queue_key) {
        return;
    }

    if let Some(rest) = specifier.strip_prefix('#') {
        // `#` alone or `#/...` is not a valid imports key.
        if rest.is_empty() || !opts.resolve_imports {
            return;
        }
        queued_specifiers.insert(queue_key);
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

    if resolved_by_path_mapping(specifier, opts) {
        return;
    }

    if let Some((package_name, subpath)) = parse_package_specifier(specifier) {
        queued_specifiers.insert(queue_key);
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
    fn test_declaration_candidates_substitute_runtime_js_flavor() {
        // tsc probes source extensions before declarations, flavor-matched.
        assert_eq!(
            declaration_candidates(PathBuf::from("pkg/subpath.js")),
            vec![
                PathBuf::from("pkg/subpath.ts"),
                PathBuf::from("pkg/subpath.tsx"),
                PathBuf::from("pkg/subpath.d.ts"),
            ]
        );
        assert_eq!(
            declaration_candidates(PathBuf::from("pkg/subpath.cjs")),
            vec![
                PathBuf::from("pkg/subpath.cts"),
                PathBuf::from("pkg/subpath.d.cts"),
            ]
        );
        assert_eq!(
            declaration_candidates(PathBuf::from("pkg/subpath.mjs")),
            vec![
                PathBuf::from("pkg/subpath.mts"),
                PathBuf::from("pkg/subpath.d.mts"),
            ]
        );
    }

    #[test]
    fn test_declaration_candidates_keep_explicit_typescript_source() {
        assert_eq!(
            declaration_candidates(PathBuf::from("pkg/src/index.ts")),
            vec![PathBuf::from("pkg/src/index.ts")]
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
