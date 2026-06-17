//! Named-type resolution memoization and declaration resolution keys.

use super::*;

use std::path::Path;

use surge_ts_types::Type;

use crate::context::{
    CheckerContext, DeclarationNamespace, DeclarationResolutionKey, DeclarationResolutionState,
    GenericInstantiationCacheEntry,
};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::TypeDeclarationInfo;

pub(crate) fn type_declaration_resolution_key(
    declaration: &TypeDeclarationInfo,
) -> DeclarationResolutionKey {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&alias.file_name),
            name: alias.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
        TypeDeclarationInfo::Interface(interface) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&interface.file_name),
            name: interface.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
    }
}

pub(crate) fn declaration_resolution_key(file_name: &str, name: &str) -> DeclarationResolutionKey {
    DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(file_name),
        name: name.to_string(),
        namespace: DeclarationNamespace::Type,
    }
}

pub(crate) fn get_cached_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolving: &[DeclarationResolutionKey],
) -> Option<ResolvedType> {
    let cache = ctx.resolved_named_types.lock().ok()?;

    match cache.get(key) {
        Some(DeclarationResolutionState::Resolved { ty, had_error }) => Some(ResolvedType {
            ty: ty.clone(),
            had_error: *had_error,
        }),
        Some(DeclarationResolutionState::Resolving) => {
            if resolving.iter().any(|current| current == key) {
                None
            } else {
                Some(ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                })
            }
        }
        None => None,
    }
}

pub(crate) fn mark_named_type_resolution_in_progress(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        cache.insert(key.clone(), DeclarationResolutionState::Resolving);
    }
}

pub(crate) fn cache_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolved: &ResolvedType,
) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        cache.insert(
            key.clone(),
            DeclarationResolutionState::Resolved {
                ty: resolved.ty.clone(),
                had_error: resolved.had_error,
            },
        );
    }
}

/// Upper bound on distinct instantiations memoized per generic declaration. The
/// hot library types resolve to a handful of top-level argument sets; the cap is
/// a defensive guard against a pathological declaration accumulating an
/// unbounded bucket that linear-search would have to scan.
const GENERIC_INSTANTIATION_BUCKET_CAP: usize = 64;

pub(crate) fn get_persistent_generic_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
) -> Option<ResolvedType> {
    let cache = ctx.program_resolved_generic_types.lock().ok()?;
    let bucket = cache.get(key)?;
    bucket.iter().find_map(|entry| {
        (entry.arguments == arguments).then(|| ResolvedType {
            ty: entry.ty.clone(),
            had_error: entry.had_error,
        })
    })
}

pub(crate) fn cache_persistent_generic_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: Vec<Type>,
    resolved: &ResolvedType,
) {
    if let Ok(mut cache) = ctx.program_resolved_generic_types.lock() {
        let bucket = cache.entry(key.clone()).or_default();
        if bucket.iter().any(|entry| entry.arguments == arguments) {
            return;
        }
        if bucket.len() >= GENERIC_INSTANTIATION_BUCKET_CAP {
            return;
        }
        bucket.push(GenericInstantiationCacheEntry {
            arguments,
            ty: resolved.ty.clone(),
            had_error: resolved.had_error,
        });
    }
}

pub(crate) fn canonical_declaration_file_name(file_name: &str) -> String {
    canonicalize_if_exists_string(Path::new(file_name))
}
