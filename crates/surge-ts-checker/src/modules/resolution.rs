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
    if !is_relative_specifier(specifier) {
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
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}
