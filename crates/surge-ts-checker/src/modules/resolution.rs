//! Relative module specifier resolution and filesystem-path candidate logic.

use super::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::paths::{canonicalize_if_exists_string, normalize_path_string};
use crate::program::ParsedProgramFile;
use crate::symbols::TypeDeclarationScope;

pub(crate) fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with(".\\")
        || specifier.starts_with("..\\")
}

pub(crate) fn is_external_specifier(specifier: &str) -> bool {
    !is_relative_specifier(specifier)
}

thread_local! {
    // Per-run memoization of relative-module resolution. Within a single check
    // the file set (`program_files` / `file_index_by_identity`) is fixed, so a
    // given (importer, specifier) always resolves the same way. The multi-pass
    // import/export binding fixpoint resolves the same specifiers 3-5 times and
    // each miss rebuilds candidate path strings and probes them with
    // `canonicalize` (realpath syscalls), so caching removes both the recompute
    // and the syscalls on passes after the first. Resolved indices are
    // run-specific, so the cache is cleared at the start of each check.
    static RELATIVE_MODULE_CACHE: RefCell<HashMap<(String, String), Option<ModuleResolution>>> =
        RefCell::new(HashMap::new());
}

/// Under node16/nodenext resolution, the files whose imports resolve in ESM
/// mode. Set for the length of one program check; every checking thread
/// resolves through it, which is why it is not thread-local.
static NODE_ESM_FILES: std::sync::RwLock<Option<Arc<std::collections::HashSet<String>>>> =
    std::sync::RwLock::new(None);

pub(crate) fn set_node_esm_files(files: Option<Arc<std::collections::HashSet<String>>>) {
    if let Ok(mut policy) = NODE_ESM_FILES.write() {
        *policy = files;
    }
}

/// `allowArbitraryExtensions` of the program being checked, read by the
/// relative resolver on every checking thread.
static ALLOW_ARBITRARY_EXTENSIONS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_allow_arbitrary_extensions(allowed: bool) {
    ALLOW_ARBITRARY_EXTENSIONS.store(allowed, std::sync::atomic::Ordering::Relaxed);
}

/// tsc's `GetResolutionDiagnostic` for a `.d.{extension}.ts` resolution: it
/// needs `allowArbitraryExtensions` unless the importer is a declaration file.
pub(crate) fn arbitrary_extension_resolution_allowed(importer_file_name: &str) -> bool {
    ALLOW_ARBITRARY_EXTENSIONS.load(std::sync::atomic::Ordering::Relaxed)
        || is_declaration_file_name(importer_file_name)
}

/// tsc's `tryAddingExtensions` for an extension it does not know: `./x.html`
/// names the declaration file `./x.d.html.ts`.
pub(crate) fn arbitrary_extension_declaration(joined: &str, specifier: &str) -> Option<String> {
    if super::candidates::classify_relative_specifier(specifier)
        != super::candidates::RelativeSpecifierShape::Extensionless
    {
        return None;
    }
    let (directory, base) = match joined.rsplit_once('/') {
        Some((directory, base)) => (Some(directory), base),
        None => (None, joined),
    };
    let (stem, extension) = base.rsplit_once('.')?;
    if extension.is_empty() || is_declaration_file_name(base) {
        return None;
    }
    let file = format!("{stem}.d.{extension}.ts");
    Some(match directory {
        Some(directory) => format!("{directory}/{file}"),
        None => file,
    })
}

/// tsc's ESM-mode rule under node16/nodenext: a relative import whose last
/// segment has no extension does not resolve (TS2834/TS2835 report it).
pub(crate) fn is_unresolvable_extensionless_esm_import(importer_file_name: &str, specifier: &str) -> bool {
    let last_segment = specifier.rsplit(['/', '\\']).next().unwrap_or(specifier);
    if last_segment.contains('.') {
        return false;
    }
    resolves_in_node_esm_mode(importer_file_name)
}

/// Whether node16/nodenext resolves this file's imports in ESM mode, where a
/// relative specifier gets no extension appended and no directory lookup.
pub(crate) fn resolves_in_node_esm_mode(importer_file_name: &str) -> bool {
    NODE_ESM_FILES
        .read()
        .ok()
        .and_then(|policy| policy.as_ref().map(|files| files.contains(importer_file_name)))
        .unwrap_or(false)
}

/// Clears the per-thread relative-module resolution cache. Called at the start
/// of a program check so resolved indices from a prior run are never reused.
pub(crate) fn clear_relative_module_cache() {
    RELATIVE_MODULE_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn resolve_relative_module(
    importer_file_name: &str,
    specifier: &str,
    program_files: &[ParsedProgramFile],
    file_index_by_identity: &surge_ts_types::fx::FxHashMap<Arc<str>, usize>,
) -> Option<ModuleResolution> {
    if !is_relative_specifier(specifier)
        || is_unresolvable_extensionless_esm_import(importer_file_name, specifier)
    {
        return None;
    }

    let cache_key = (importer_file_name.to_string(), specifier.to_string());
    if let Some(cached) =
        RELATIVE_MODULE_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned())
    {
        return cached;
    }

    let resolved = resolve_relative_module_uncached(
        importer_file_name,
        specifier,
        program_files,
        file_index_by_identity,
    );
    RELATIVE_MODULE_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, resolved.clone());
    });
    resolved
}

pub(crate) fn resolve_relative_module_uncached(
    importer_file_name: &str,
    specifier: &str,
    program_files: &[ParsedProgramFile],
    file_index_by_identity: &surge_ts_types::fx::FxHashMap<Arc<str>, usize>,
) -> Option<ModuleResolution> {
    let importer_dir = module_directory(importer_file_name);
    let normalized_specifier = normalize_path_string(specifier);
    let joined_specifier = if importer_dir.is_empty() {
        normalized_specifier.clone()
    } else {
        normalize_path_string(&format!("{importer_dir}/{normalized_specifier}"))
    };

    if let Some(declaration) = arbitrary_extension_declaration(&joined_specifier, &normalized_specifier)
        && let Some(resolved_file_index) =
            file_index_by_identity.get(canonical_file_identity(&declaration).as_str())
    {
        if !arbitrary_extension_resolution_allowed(importer_file_name) {
            return None;
        }
        return Some(ModuleResolution {
            resolved_file_index: *resolved_file_index,
            resolved_file_name: program_files[*resolved_file_index].file_name.clone(),
        });
    }

    let candidate_paths =
        super::candidates::relative_import_candidates(&joined_specifier, &normalized_specifier)?;

    for candidate in candidate_paths {
        let candidate = canonical_file_identity(&candidate);
        if let Some(resolved_file_index) = file_index_by_identity.get(candidate.as_str()) {
            return Some(ModuleResolution {
                resolved_file_index: *resolved_file_index,
                resolved_file_name: program_files[*resolved_file_index].file_name.clone(),
            });
        }
    }

    None
}

#[allow(dead_code)]
pub(crate) fn resolve_relative_local_type_scope(
    importer_file_name: &str,
    module_specifier: &str,
    program_files: &[ParsedProgramFile],
    file_index_by_identity: &surge_ts_types::fx::FxHashMap<Arc<str>, usize>,
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
) -> Option<(usize, Arc<TypeDeclarationScope>)> {
    let resolved = resolve_relative_module(
        importer_file_name,
        module_specifier,
        program_files,
        file_index_by_identity,
    )?;
    let local_scope = module_resolution_scopes
        .get(resolved.resolved_file_index)
        .and_then(|scope| scope.clone())?;
    Some((resolved.resolved_file_index, local_scope))
}

pub(crate) fn module_directory(file_name: &str) -> String {
    let normalized = normalize_path_string(file_name);
    normalized
        .rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_default()
}

pub(crate) fn canonical_file_identity(file_name: &str) -> String {
    crate::program::record_canonical_file_id_lookup();
    canonicalize_if_exists_string(Path::new(file_name))
}

pub(crate) fn is_declaration_file_name(file_name: &str) -> bool {
    surge_ts_syntax::is_declaration_file_name(file_name)
}
