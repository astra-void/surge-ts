use std::collections::HashSet;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedType, ParsedTypeParameter};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::TypeDeclarationInfo;

pub(crate) mod cache;
mod diagnostics;
mod interface;
mod resolve;
mod utility;

pub(crate) use cache::*;
pub(crate) use diagnostics::*;
pub(crate) use interface::*;
pub(crate) use resolve::*;
pub(crate) use utility::*;
/// Type-parameter bindings for one resolution context.
///
/// Substitutions are tiny (one entry per type parameter of the declaration
/// being instantiated) but cloned constantly — every lazy reference captures
/// one — so the maps are name-sorted `Arc`-shared vectors: a clone is two
/// refcount bumps, a copy-on-write is a single allocation of small pairs, and
/// iteration order matches the previous `BTreeMap` exactly (sorted by name).
#[derive(Debug, Clone, Default)]
pub(crate) struct TypeParameterSubstitution {
    values: Option<Arc<Vec<(Arc<str>, Type)>>>,
    placeholders: Option<Arc<Vec<Arc<str>>>>,
}

impl TypeParameterSubstitution {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }

    /// Census-only `(address, bytes)` pairs for the `Arc`-shared capture maps,
    /// so retention walks charge each shared map once.
    pub(crate) fn census_shared_captures(&self) -> Vec<(usize, u64)> {
        let mut captures = Vec::new();
        if let Some(values) = &self.values {
            let bytes = values
                .iter()
                .map(|(name, _)| {
                    (name.len()
                        + std::mem::size_of::<Arc<str>>()
                        + std::mem::size_of::<Type>()
                        + 32) as u64
                })
                .sum();
            captures.push((Arc::as_ptr(values) as *const () as usize, bytes));
        }
        if let Some(placeholders) = &self.placeholders {
            let bytes = placeholders
                .iter()
                .map(|name| (name.len() + std::mem::size_of::<Arc<str>>() + 16) as u64)
                .sum();
            captures.push((Arc::as_ptr(placeholders) as *const () as usize, bytes));
        }
        captures
    }
}

impl TypeParameterSubstitution {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set(&mut self, name: String, ty: Type, placeholder: bool) {
        let name: Arc<str> = Arc::from(name.as_str());
        let values = Arc::make_mut(self.values.get_or_insert_with(|| Arc::new(Vec::new())));
        match values.binary_search_by(|(existing, _)| existing.as_ref().cmp(&*name)) {
            Ok(index) => values[index].1 = ty,
            Err(index) => values.insert(index, (name.clone(), ty)),
        }
        if placeholder {
            let placeholders = Arc::make_mut(
                self.placeholders
                    .get_or_insert_with(|| Arc::new(Vec::new())),
            );
            if let Err(index) =
                placeholders.binary_search_by(|existing| existing.as_ref().cmp(&*name))
            {
                placeholders.insert(index, name);
            }
        } else if let Some(placeholders) = self.placeholders.as_mut() {
            if let Ok(index) =
                placeholders.binary_search_by(|existing| existing.as_ref().cmp(&*name))
            {
                Arc::make_mut(placeholders).remove(index);
            }
        }
    }

    pub(crate) fn insert(&mut self, name: String, ty: Type) {
        self.set(name, ty, false);
    }

    pub(crate) fn insert_placeholder(&mut self, name: String, ty: Type) {
        self.set(name, ty, true);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Type> {
        let values = self.values.as_deref()?;
        values
            .binary_search_by(|(existing, _)| existing.as_ref().cmp(name))
            .ok()
            .map(|index| &values[index].1)
    }

    pub(crate) fn is_placeholder(&self, name: &str) -> bool {
        self.placeholders.as_deref().is_some_and(|placeholders| {
            placeholders
                .binary_search_by(|existing| existing.as_ref().cmp(name))
                .is_ok()
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &Type)> {
        self.values
            .iter()
            .flat_map(|values| values.iter().map(|(name, ty)| (name, ty)))
    }

    pub(crate) fn extend(&mut self, other: Self) {
        let Self {
            values,
            placeholders,
        } = other;
        let Some(values) = values else {
            return;
        };
        let values = Arc::try_unwrap(values).unwrap_or_else(|values| (*values).clone());
        let placeholders = placeholders
            .map(|placeholders| {
                Arc::try_unwrap(placeholders).unwrap_or_else(|placeholders| (*placeholders).clone())
            })
            .unwrap_or_default();

        for (name, ty) in values {
            let is_placeholder = placeholders
                .binary_search_by(|existing| existing.as_ref().cmp(&*name))
                .is_ok();
            self.set(name.as_ref().to_string(), ty, is_placeholder);
        }
    }
}

pub(crate) fn report_duplicate_type_parameters(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let mut seen = HashSet::new();

    for type_parameter in type_parameters {
        if !seen.insert(type_parameter.name.clone()) {
            let mut diagnostic = Diagnostic::surge_duplicate_type_parameter(
                type_parameter.name.clone(),
                ctx.file_name.clone(),
            );

            if let Some(span) = type_parameter.name_span.or(type_parameter.span) {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedType {
    ty: Type,
    had_error: bool,
}

pub(crate) fn map_parsed_type(parsed_type: ParsedType, ctx: &mut CheckerContext) -> Type {
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        map_parsed_type_with_substitution(parsed_type, ctx, &TypeParameterSubstitution::new())
    })
}

pub(crate) fn map_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> Type {
    let mut resolving = Vec::new();
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(
            parsed_type,
            ctx,
            &mut resolving,
            &merged_type_parameter_substitution(ctx, substitution),
        )
        .ty
    })
}

fn merged_type_parameter_substitution(
    ctx: &CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> TypeParameterSubstitution {
    let mut merged = TypeParameterSubstitution::new();

    for scope in &ctx.type_parameter_scopes {
        for (name, ty) in scope {
            merged.insert_placeholder(name.clone(), ty.clone());
        }
    }

    merged.extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged));

    merged
}

pub(crate) fn validate_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    ctx: &mut CheckerContext,
) {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &alias.body.type_parameters {
                substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
            }

            let mut resolving = Vec::new();
            with_type_declaration_scope(&alias.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &alias.file_name, |ctx| {
                    // Register the parameter constraints so indexed access through
                    // a constrained parameter (`T extends …`) is not falsely
                    // flagged. Placeholder detection still flows through the
                    // substitution above.
                    ctx.push_type_parameter_scope(&alias.body.type_parameters, None);
                    resolve_parsed_type_with_substitution(
                        alias.body.ty.clone(),
                        ctx,
                        &mut resolving,
                        &substitution,
                    );
                    ctx.pop_type_parameter_scope();
                })
            });
        }
        TypeDeclarationInfo::Interface(interface) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &interface.body.type_parameters {
                substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
            }

            let mut resolving = Vec::new();
            with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &interface.file_name, |ctx| {
                    ctx.push_type_parameter_scope(&interface.body.type_parameters, None);
                    resolve_interface_declaration(
                        &interface.body.extends,
                        &interface.body.members,
                        interface.body.string_index_type.as_ref(),
                        interface.body.call_signature.as_ref(),
                        &interface.body.construct_signatures,
                        ctx,
                        &mut resolving,
                        &substitution,
                        None,
                        None,
                        None,
                    );
                    ctx.pop_type_parameter_scope();
                })
            });
        }
    }
}

fn with_file_name<R>(
    ctx: &mut CheckerContext,
    file_name: &str,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let current_file_name = ctx.file_name.clone();
    let crossed_file = current_file_name != file_name;
    ctx.set_file_name(file_name.to_string());
    if crossed_file {
        ctx.cross_file_resolution_depth += 1;
    }
    let result = f(ctx);
    if crossed_file {
        ctx.cross_file_resolution_depth -= 1;
    }
    ctx.set_file_name(current_file_name);
    result
}

pub(crate) fn with_type_declaration_scope<R>(
    type_declaration_scope: &Option<Arc<crate::symbols::TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let saved_type_declaration_scope = ctx.type_declaration_scope.clone();

    if let Some(type_declaration_scope) = type_declaration_scope {
        ctx.type_declaration_scope = Some(type_declaration_scope.clone());
    }

    let result = f(ctx);
    ctx.type_declaration_scope = saved_type_declaration_scope;
    result
}
