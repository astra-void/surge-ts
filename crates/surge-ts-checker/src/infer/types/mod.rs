use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedType, ParsedTypeParameter};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::TypeDeclarationInfo;

mod cache;
mod diagnostics;
mod interface;
mod resolve;
mod utility;

pub(crate) use cache::*;
pub(crate) use diagnostics::*;
pub(crate) use interface::*;
pub(crate) use resolve::*;
pub(crate) use utility::*;
#[derive(Debug, Clone, Default)]
pub(crate) struct TypeParameterSubstitution {
    values: BTreeMap<String, Type>,
    placeholders: HashSet<String>,
}

impl TypeParameterSubstitution {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }
}

impl TypeParameterSubstitution {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set(&mut self, name: String, ty: Type, placeholder: bool) {
        self.values.insert(name.clone(), ty);
        if placeholder {
            self.placeholders.insert(name);
        } else {
            self.placeholders.remove(&name);
        }
    }

    pub(crate) fn insert(&mut self, name: String, ty: Type) {
        self.set(name, ty, false);
    }

    pub(crate) fn insert_placeholder(&mut self, name: String, ty: Type) {
        self.set(name, ty, true);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Type> {
        self.values.get(name)
    }

    pub(crate) fn is_placeholder(&self, name: &str) -> bool {
        self.placeholders.contains(name)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &Type)> {
        self.values.iter()
    }

    pub(crate) fn extend(&mut self, other: Self) {
        let Self {
            values,
            placeholders,
        } = other;

        for (name, ty) in values {
            if placeholders.contains(&name) {
                self.insert_placeholder(name, ty);
            } else {
                self.insert(name, ty);
            }
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
                    resolve_parsed_type_with_substitution(
                        alias.body.ty.clone(),
                        ctx,
                        &mut resolving,
                        &substitution,
                    )
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
                    resolve_interface_declaration(
                        &interface.body.extends,
                        &interface.body.members,
                        interface.body.string_index_type.as_ref(),
                        interface.body.call_signature.as_ref(),
                        ctx,
                        &mut resolving,
                        &substitution,
                    )
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
    ctx.set_file_name(file_name.to_string());
    let result = f(ctx);
    ctx.set_file_name(current_file_name);
    result
}

fn with_type_declaration_scope<R>(
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
