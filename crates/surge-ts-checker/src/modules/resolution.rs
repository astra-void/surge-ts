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
    file_index_by_identity: &HashMap<Arc<str>, usize>,
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
    file_index_by_identity: &HashMap<Arc<str>, usize>,
) -> Option<ModuleResolution> {
    let importer_dir = module_directory(importer_file_name);
    let normalized_specifier = normalize_path_string(specifier);
    let joined_specifier = if importer_dir.is_empty() {
        normalized_specifier.clone()
    } else {
        normalize_path_string(&format!("{importer_dir}/{normalized_specifier}"))
    };

    let candidate_paths = match relative_specifier_kind(&normalized_specifier) {
        RelativeSpecifierKind::ExplicitTs => vec![joined_specifier],
        RelativeSpecifierKind::ExplicitJs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".ts", ".tsx"],
                &[".d.ts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitMjs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".mts"],
                &[".d.mts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitCjs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".cts"],
                &[".d.cts"],
            ));
            candidates
        }
        RelativeSpecifierKind::Extensionless => relative_resolution_candidates(&joined_specifier),
        RelativeSpecifierKind::Unsupported => return None,
    };

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
    file_index_by_identity: &HashMap<Arc<str>, usize>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelativeSpecifierKind {
    ExplicitTs,
    ExplicitJs,
    ExplicitMjs,
    ExplicitCjs,
    Extensionless,
    Unsupported,
}

pub(crate) fn relative_specifier_kind(specifier: &str) -> RelativeSpecifierKind {
    let last_segment = specifier.rsplit('/').next().unwrap_or(specifier);

    if last_segment == "." || last_segment == ".." {
        return RelativeSpecifierKind::Extensionless;
    }

    if last_segment.ends_with(".tsx")
        || last_segment.ends_with(".jsx")
        || last_segment.ends_with(".mts")
        || last_segment.ends_with(".cts")
        || last_segment.ends_with(".d.ts")
        || last_segment.ends_with(".d.mts")
        || last_segment.ends_with(".d.cts")
        || last_segment.ends_with(".json")
    {
        return RelativeSpecifierKind::Unsupported;
    }

    if last_segment.ends_with(".ts") {
        return RelativeSpecifierKind::ExplicitTs;
    }

    if last_segment.ends_with(".js") {
        return RelativeSpecifierKind::ExplicitJs;
    }

    if last_segment.ends_with(".mjs") {
        return RelativeSpecifierKind::ExplicitMjs;
    }

    if last_segment.ends_with(".cjs") {
        return RelativeSpecifierKind::ExplicitCjs;
    }

    RelativeSpecifierKind::Extensionless
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

pub(crate) fn relative_resolution_candidates(base: &str) -> Vec<String> {
    vec![
        base.to_string(),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.d.ts"),
        format!("{base}.mts"),
        format!("{base}.cts"),
        format!("{base}.d.mts"),
        format!("{base}.d.cts"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.d.ts"),
        format!("{base}/index.mts"),
        format!("{base}/index.cts"),
        format!("{base}/index.d.mts"),
        format!("{base}/index.d.cts"),
    ]
}

pub(crate) fn relative_resolution_candidates_with_js_substitution(
    base: &str,
    source_extensions: &[&str],
    declaration_extensions: &[&str],
) -> Vec<String> {
    let mut candidates = Vec::new();

    for extension in source_extensions {
        candidates.push(format!("{base}{extension}"));
    }
    for extension in declaration_extensions {
        candidates.push(format!("{base}{extension}"));
    }

    candidates.push(format!("{base}/index.ts"));
    candidates.push(format!("{base}/index.tsx"));
    candidates.push(format!("{base}/index.d.ts"));
    candidates.push(format!("{base}/index.mts"));
    candidates.push(format!("{base}/index.cts"));
    candidates.push(format!("{base}/index.d.mts"));
    candidates.push(format!("{base}/index.d.cts"));

    candidates
}

pub(crate) fn strip_extension(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}

pub(crate) fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}
