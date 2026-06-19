//! Named-type resolution memoization and declaration resolution keys.

use super::*;

use std::path::Path;
use std::sync::Arc;

use surge_ts_types::{ResolveReference, Type, TypeReference};

use crate::context::{
    CheckerContext, DeclarationNamespace, DeclarationResolutionKey, DeclarationResolutionState,
    GenericInstantiationCacheEntry, InstantiationCacheEntry,
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

/// Resolver for a lazy [`Type::Reference`] that resolves to an already-computed,
/// program-wide-shared structural expansion. The expansion is computed once per
/// unique instantiation by [`intern_instantiation`] and shared via `Arc`, so
/// resolving the reference never re-expands the declaration body.
#[derive(Debug)]
struct InternedInstantiation {
    resolved: Arc<Type>,
}

impl ResolveReference for InternedInstantiation {
    fn resolve(&self) -> Type {
        (*self.resolved).clone()
    }
}

/// Interns the structural expansion of `key` at `arguments`, returning the
/// shared `Arc<Type>`. On a hit the previously-expanded shape is returned and
/// `structural` is discarded, so each unique instantiation expands at most once.
pub(crate) fn intern_instantiation(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    structural: Type,
) -> Arc<Type> {
    let Ok(mut cache) = ctx.program_instantiations.lock() else {
        return Arc::new(structural);
    };
    let bucket = cache.entry(key.clone()).or_default();
    if let Some(entry) = bucket.iter().find(|entry| entry.arguments == arguments) {
        return entry.resolved.clone();
    }
    let resolved = Arc::new(structural);
    if bucket.len() < GENERIC_INSTANTIATION_BUCKET_CAP {
        bucket.push(InstantiationCacheEntry {
            arguments: arguments.to_vec(),
            resolved: resolved.clone(),
        });
    }
    resolved
}

/// Looks up a previously-interned instantiation without expanding anything.
pub(crate) fn lookup_instantiation(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
) -> Option<InstantiationCacheEntry> {
    let cache = ctx.program_instantiations.lock().ok()?;
    cache
        .get(key)?
        .iter()
        .find(|entry| entry.arguments == arguments)
        .cloned()
}

/// Builds a lazy/nominal [`Type::Reference`] over a shared structural expansion.
/// `id` is the nominal identity (`file\u{0}name`), `display` the diagnostic form
/// (e.g. `Box<string>`), and `arguments` the resolved type arguments.
#[allow(dead_code)]
pub(crate) fn make_type_reference(
    id: impl Into<Arc<str>>,
    display: impl Into<Arc<str>>,
    arguments: Vec<Type>,
    resolved: Arc<Type>,
) -> Type {
    Type::Reference(TypeReference::new(
        id,
        display,
        arguments,
        Arc::new(InternedInstantiation { resolved }),
    ))
}
