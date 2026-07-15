use super::*;

use surge_ts_syntax::{ParsedTypeParameter, TextSpan};

pub(crate) struct BoundTypeArguments {
    pub(crate) substitution: TypeParameterSubstitution,
    /// Whether any argument/default resolved with `had_error`. The binding
    /// still proceeds with the degraded type — one failed argument must not
    /// erase an otherwise-usable instantiation (a callable's degraded callback
    /// parameter would otherwise strip the whole call signature) — but the
    /// taint must reach the caller's `ResolvedType` so degraded expansions are
    /// never interned.
    pub(crate) had_error: bool,
}

pub(crate) fn bind_type_arguments(
    type_parameters: &[ParsedTypeParameter],
    type_arguments: Vec<ParsedType>,
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    parent_substitution: &TypeParameterSubstitution,
    pre_resolved: Option<&[Type]>,
    declaration_scope: Option<(
        &Option<std::sync::Arc<crate::symbols::TypeDeclarationScope>>,
        &str,
    )>,
) -> Option<BoundTypeArguments> {
    let mut bound_had_error = false;
    if type_parameters.is_empty() {
        if !type_arguments.is_empty() {
            emit_type_is_not_generic(name, name_span, ctx);
            return None;
        }

        return Some(BoundTypeArguments {
            substitution: TypeParameterSubstitution::new(),
            had_error: false,
        });
    }

    if type_arguments.len() > type_parameters.len() {
        emit_generic_arity(name, type_parameters.len(), name_span, ctx);
        return None;
    }

    let mut substitution = TypeParameterSubstitution::new();

    for (index, parameter) in type_parameters.iter().enumerate() {
        if let Some(argument) = type_arguments.get(index) {
            // Reuse the caller's already-resolved argument when available instead
            // of resolving the `ParsedType` a second time. The redundant
            // resolution is exponential on deeply nested generics (each level
            // re-resolves its arguments), so reusing the probe result is what keeps
            // a nominal-reference instantiation linear.
            let resolved_ty = if let Some(pre) = pre_resolved.and_then(|pre| pre.get(index)) {
                pre.clone()
            } else {
                let resolved_argument =
                    resolve_parsed_type(argument.clone(), ctx, resolving, parent_substitution);
                bound_had_error |= resolved_argument.had_error;
                resolved_argument.ty
            };

            if parsed_type_is_placeholder_reference(argument, parent_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), resolved_ty);
            } else {
                substitution.insert(parameter.name.clone(), resolved_ty);
            }
            continue;
        }

        let Some(default_type) = parameter.default_type.clone() else {
            emit_generic_arity(name, type_parameters.len(), name_span, ctx);
            return None;
        };

        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let default_type_is_placeholder =
            parsed_type_is_placeholder_reference(&default_type, &effective_substitution);
        // A type-parameter default (`T extends X = X`) is authored in the
        // *declaring* module: resolve it under that module's scope and file, not
        // the consumer's, so an imported generic alias/interface binds its
        // defaults even when they name non-exported siblings.
        let resolved_default = if let Some((scope, file_name)) = declaration_scope {
            with_type_declaration_scope(scope, ctx, |ctx| {
                with_file_name(ctx, file_name, |ctx| {
                    resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
                })
            })
        } else {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        };
        bound_had_error |= resolved_default.had_error;

        if default_type_is_placeholder {
            substitution.insert_placeholder(parameter.name.clone(), resolved_default.ty);
        } else {
            substitution.insert(parameter.name.clone(), resolved_default.ty);
        }
    }

    Some(BoundTypeArguments {
        substitution,
        had_error: bound_had_error,
    })
}

pub(crate) fn extend_substitution_with_type_parameters(
    parent_substitution: &TypeParameterSubstitution,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> TypeParameterSubstitution {
    let mut substitution =
        parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);

    for parameter in type_parameters {
        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let resolved = parameter.default_type.clone().map(|default_type| {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        });

        let ty = match resolved {
            Some(resolved) if !resolved.had_error => resolved.ty,
            Some(_) => Type::Unknown,
            None => Type::Unknown,
        };

        if let Some(default_type) = parameter.default_type.as_ref() {
            if parsed_type_is_placeholder_reference(default_type, &effective_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), ty);
                continue;
            }
        }

        substitution.insert(parameter.name.clone(), ty);
    }

    substitution
}

pub(crate) fn parsed_type_is_placeholder_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name)
    )
}

pub(crate) fn parsed_type_placeholder_name<'a>(
    parsed_type: &'a ParsedType,
    substitution: &TypeParameterSubstitution,
) -> Option<&'a str> {
    match parsed_type {
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name) => {
            Some(named_type.name.as_str())
        }
        _ => None,
    }
}

pub(crate) fn is_concrete_substituted_named_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type)
            if substitution
                .get(&named_type.name)
                .is_some()
                && !substitution.is_placeholder(&named_type.name)
    )
}

pub(crate) fn is_concrete_substituted_index_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    match parsed_type {
        ParsedType::Named(named_type) => {
            substitution.get(&named_type.name).is_some()
                && !substitution.is_placeholder(&named_type.name)
        }
        ParsedType::KeyOf(inner) => {
            is_concrete_substituted_named_reference(inner.as_ref(), substitution)
        }
        _ => false,
    }
}
