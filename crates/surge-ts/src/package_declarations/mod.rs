use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use surge_ts_checker::SourceFileInput;
use surge_ts_config::canonicalize_if_exists_string;
use surge_ts_syntax::{
    ReferenceTypeDirective, TextSpan, extract_reference_path_directives,
    extract_reference_type_directives,
};

use crate::package_resolution::{
    ResolverOptions, select_export_target, select_export_targets, select_import_targets,
    types_versions_candidates,
};
use crate::specifier::{is_external_specifier, is_relative_specifier};

mod helpers;
use helpers::*;

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
    // Fx-hashed: these sit on the per-(importer, specifier) resolution path,
    // and SipHash over `PathBuf`/multi-`String` keys showed up in profiles.
    // Neither map is iterated, so the hasher cannot affect output order.
    package_json_cache:
        surge_ts_types::fx::FxHashMap<PathBuf, Option<std::sync::Arc<serde_json::Value>>>,
    entrypoint_cache: surge_ts_types::fx::FxHashMap<
        PackageEntrypointCacheKey,
        Option<PackageEntrypointResolution>,
    >,
    /// Count of `sources` entries whose specifiers this resolver has already
    /// queued, so loader fixpoint iterations scan each source exactly once.
    scanned_sources: usize,
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

/// Package-declaration resolutions in BFS resolution order:
/// `(canonical importer file, specifier, canonical resolved file)`.
///
/// The same bare specifier may resolve differently from different importers
/// (nested `node_modules`, `#imports` scopes, self-name imports), so results
/// are keyed by importer rather than flattened to `specifier → file`. The
/// ordered `Vec` keeps the project-wide first-resolution fallback map
/// deterministic when the caller merges.
pub(crate) type PackageResolutions = Vec<(String, String, String)>;

#[allow(dead_code)]
pub fn resolve_package_declaration_entrypoints(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
) -> PackageResolutions {
    let mut cache = PackageDeclarationResolverCache::default();
    let mut scanner = crate::specifier_scan::ModuleSpecifierScanner::new();
    resolve_package_declaration_entrypoints_with_cache(
        inputs,
        sources,
        root_dir,
        &ResolverOptions::default(),
        &mut cache,
        &mut scanner,
    )
}

pub(crate) fn resolve_package_declaration_entrypoints_with_cache(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    opts: &ResolverOptions,
    cache: &mut PackageDeclarationResolverCache,
    scanner: &mut crate::specifier_scan::ModuleSpecifierScanner,
) -> PackageResolutions {
    let mut packages_to_resolve: VecDeque<PackageDeclarationRequest> = VecDeque::new();
    let mut resolutions: PackageResolutions = Vec::new();
    let mut resolved_packages: HashSet<(String, String)> = HashSet::new();
    let mut known_file_names: HashSet<String> = inputs
        .iter()
        .map(|input| canonicalize_if_exists_string(Path::new(&input.file_name)))
        .collect();
    let mut queued_specifiers: HashSet<(String, String)> = HashSet::new();

    scanner.prefetch(sources, cache.scanned_sources);
    for index in cache.scanned_sources..sources.len() {
        let (file_path, file_name, source_text) = {
            let (file_path, file_name, source_text) = &sources[index];
            (file_path.clone(), file_name.clone(), source_text.clone())
        };
        let importer_dir = file_path.parent().unwrap_or(root_dir).to_path_buf();
        let specifiers = scanner.specifiers(index, &file_name, &source_text);
        extract_packages_from_source(
            &specifiers,
            &file_path.to_string_lossy(),
            &importer_dir,
            opts,
            &mut packages_to_resolve,
            &mut queued_specifiers,
        );
    }

    // The queue is finite: every (importer file, specifier) pair is enqueued at
    // most once, and newly loaded files enqueue only their own pairs.
    while let Some(req) = packages_to_resolve.pop_front() {
        let importer_key = canonicalize_if_exists_string(&req.importer_file);
        if resolved_packages.contains(&(importer_key.clone(), req.specifier.clone())) {
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
                resolved_packages.insert((importer_key.clone(), req.specifier.clone()));
                resolutions.push((
                    importer_key.clone(),
                    req.specifier.clone(),
                    normalized_file_name.clone(),
                ));

                if !known_file_names.contains(&normalized_file_name) {
                    let read_start = std::time::Instant::now();
                    let Ok(source_text) = std::fs::read_to_string(&path) else {
                        continue;
                    };
                    crate::io_stats::record_package_declaration_read(
                        source_text.len(),
                        read_start.elapsed(),
                    );

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

                    let new_index = sources.len() - 1;
                    let specifiers =
                        scanner.specifiers(new_index, &normalized_file_name, &source_text);
                    let new_importer_dir = path.parent().unwrap_or(root_dir).to_path_buf();
                    extract_packages_from_source(
                        &specifiers,
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
                    resolved_packages.insert((importer_key.clone(), req.specifier.clone()));
                    resolutions.push((importer_key, req.specifier.clone(), file_name));
                }
            }
        }
    }

    // Every source present at exit has been queued (pre-existing ones by the
    // entry loop, self-loaded declarations inline above); later calls resume
    // from here.
    cache.scanned_sources = sources.len();

    resolutions
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
/// `exports["."].types` / `index.d.ts`. A name no type root provides then falls
/// through to the secondary `node_modules` lookup, which is what resolves
/// subpath directives such as `vitest/globals`.
pub(crate) fn resolve_type_packages(
    inputs: &mut Vec<SourceFileInput>,
    sources: &mut Vec<(PathBuf, String, String)>,
    root_dir: &Path,
    types: Option<&[String]>,
    type_roots: &[PathBuf],
    opts: &ResolverOptions,
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
        let resolved = resolve_type_directive_in_roots(&name, &roots, cache).or_else(|| {
            resolve_type_directive_in_node_modules(&name, root_dir, root_dir, opts, cache)
        });
        match resolved {
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
    root_dir: PathBuf,
    scanned_files: HashSet<String>,
    // The secondary `node_modules` lookup walks up from the referencing file, so
    // the same directive name can resolve differently per directory.
    resolution_cache: HashMap<(PathBuf, String), Option<PathBuf>>,
    effective_type_names: Vec<String>,
    seen_effective: HashSet<String>,
    missing: Vec<MissingReferenceTypeDirective>,
}

impl ReferenceTypeDirectiveResolver {
    pub fn new(root_dir: &Path, type_roots: &[PathBuf]) -> Self {
        Self {
            roots: effective_type_roots(root_dir, type_roots),
            root_dir: root_dir.to_path_buf(),
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
        opts: &ResolverOptions,
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
                        opts,
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
        opts: &ResolverOptions,
        cache: &mut PackageDeclarationResolverCache,
    ) {
        let name = directive.value;
        let lookup_dir = Path::new(file_name)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root_dir.clone());
        let cache_key = (lookup_dir.clone(), name.clone());
        let resolved = match self.resolution_cache.get(&cache_key) {
            Some(cached) => cached.clone(),
            None => {
                let resolved = if is_relative_specifier(&name) || Path::new(&name).is_absolute() {
                    resolve_relative_type_directive(&name, &lookup_dir, cache)
                } else {
                    resolve_type_directive_in_roots(&name, &self.roots, cache).or_else(|| {
                        resolve_type_directive_in_node_modules(
                            &name,
                            &lookup_dir,
                            &self.root_dir,
                            opts,
                            cache,
                        )
                    })
                };
                self.resolution_cache.insert(cache_key, resolved.clone());
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
