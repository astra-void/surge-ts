use super::*;

use surge_ts_syntax::{ParsedIndexedAccessType, TextSpan};

use crate::program::{
    record_generic_indexed_access_attempt, record_generic_indexed_access_invalid_key,
    record_generic_indexed_access_substituted_key,
    record_generic_indexed_access_substituted_receiver, record_generic_indexed_access_success,
    record_generic_indexed_access_unknown_fallback,
};

trait ParsedTypeSpan {
    fn span(&self) -> Option<TextSpan>;
}

impl ParsedTypeSpan for ParsedType {
    fn span(&self) -> Option<TextSpan> {
        match self {
            ParsedType::Named(named_type) => named_type.span,
            ParsedType::TypeOf(type_of) => type_of.name_span,
            ParsedType::IndexedAccess(indexed_access) => indexed_access.span,
            ParsedType::Mapped(mapped) => mapped.key_span.or(mapped.span),
            _ => None,
        }
    }
}

/// Selects the type of `index` from `object` without emitting diagnostics or
/// recording cascade errors. Used when the receiver already errored but its
/// structural shape is still usable, so the requested property can be selected
/// without cascading a fresh missing-property diagnostic. Returns `None` when
/// the receiver is not an indexable structure or the key is not present.
fn select_indexed_property_no_cascade(object: &Type, index: &Type) -> Option<Type> {
    match (object, index) {
        (Type::Object(object_type), Type::StringLiteral(key)) => {
            object_type.get_property_access_type(key)
        }
        (Type::Object(object_type), Type::Union(union_ty)) => {
            let mut types = Vec::new();
            for key_ty in union_ty.types() {
                let Type::StringLiteral(key) = key_ty else {
                    return None;
                };
                types.push(object_type.get_property_access_type(key)?);
            }
            Some(union_type(types))
        }
        (Type::Tuple(elements), Type::NumberLiteral(num)) => {
            let index = num.value.parse::<usize>().ok()?;
            elements.get(index).cloned()
        }
        (Type::Array(element_type), Type::Number) => Some(*element_type.clone()),
        (Type::Tuple(elements), Type::Number) => Some(union_type(elements.clone())),
        _ => None,
    }
}

pub(super) fn resolve_indexed_access_type(
    indexed_access: ParsedIndexedAccessType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    record_generic_indexed_access_attempt();
    let object_type_for_placeholder = indexed_access.object_type.clone();
    let object_placeholder_name =
        parsed_type_placeholder_name(object_type_for_placeholder.as_ref(), substitution);
    let index_placeholder_name =
        parsed_type_placeholder_name(indexed_access.index_type.as_ref(), substitution);
    let object_is_concrete_substitution =
        is_concrete_substituted_named_reference(object_type_for_placeholder.as_ref(), substitution);
    let index_is_concrete_substitution =
        is_concrete_substituted_index_reference(indexed_access.index_type.as_ref(), substitution);
    let generic_indexed_access = object_placeholder_name.is_some()
        || index_placeholder_name.is_some()
        || object_is_concrete_substitution
        || index_is_concrete_substitution;
    let index_is_keyof_same_placeholder = matches!(
        (
            object_placeholder_name.as_deref(),
            indexed_access.index_type.as_ref()
        ),
        (
            Some(object_name),
            ParsedType::KeyOf(inner)
        ) if matches!(
            inner.as_ref(),
            ParsedType::Named(named_type) if named_type.name == object_name
        )
    );
    // `K extends keyof T` makes the generic `T[K]` a valid index even though
    // neither side is concrete yet, so it must not cascade into TS2536.
    let index_constraint_satisfies_object = match (
        object_placeholder_name.as_deref(),
        index_placeholder_name.as_deref(),
    ) {
        (Some(object_name), Some(index_name)) => {
            ctx.type_parameter_keyof_constraint_target(index_name) == Some(object_name)
        }
        _ => false,
    };
    let index_is_valid_generic_key =
        index_is_keyof_same_placeholder || index_constraint_satisfies_object;

    // An index access through a *constrained* type parameter (`T extends …`,
    // `K extends Key`, `strict extends Boolean`, …) is validated by tsc against
    // that constraint. We do not fully resolve those (often library-generated)
    // constraints, so verifying the key here would only ever produce false
    // `TS2536`/`TS2538`s. An unconstrained `T[K]` is still a genuine error and
    // is left to the checks below.
    let involves_constrained_type_parameter = object_placeholder_name
        .as_deref()
        .is_some_and(|name| ctx.type_parameter_has_constraint(name))
        || index_placeholder_name
            .as_deref()
            .is_some_and(|name| ctx.type_parameter_has_constraint(name));

    if object_is_concrete_substitution {
        record_generic_indexed_access_substituted_receiver();
    }
    if index_is_concrete_substitution {
        record_generic_indexed_access_substituted_key();
    }

    let resolved_object =
        resolve_parsed_type(*indexed_access.object_type, ctx, resolving, substitution);
    // Peel a nominal reference receiver (`User["id"]`) to its structural object so
    // the index lookup below reads its properties instead of failing to match.
    let resolved_object = ResolvedType {
        ty: crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::IndexedAccess,
            || resolved_object.ty.peeled(),
        ),
        had_error: resolved_object.had_error,
    };

    let resolved_index = resolve_parsed_type(
        *indexed_access.index_type.clone(),
        ctx,
        resolving,
        substitution,
    );

    if resolved_object.had_error {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        // The receiver shape is known but one of its inner property types could
        // not be resolved (e.g. an imported alias whose body references a lib
        // type unavailable in the declaring module's scope). Still select the
        // requested property so downstream code sees the right type, without
        // emitting a fresh diagnostic from a receiver that already errored. The
        // selected property is a legitimate type, so it is returned clean and
        // participates normally in narrowing. A truly unresolved receiver
        // (Unknown) has no selectable property and stays no-cascade as Unknown.
        if let Some(selected) =
            select_indexed_property_no_cascade(&resolved_object.ty, &resolved_index.ty)
        {
            return ResolvedType {
                ty: selected,
                had_error: false,
            };
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    if (object_placeholder_name.is_some() && index_is_valid_generic_key)
        || involves_constrained_type_parameter
    {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // A receiver that *resolved* to `unknown` (e.g. `typeof external` whose
    // value type could not be reconstructed, or a generic alias whose body we do
    // not model) cannot have its index validated, so indexing it degrades to
    // `unknown` rather than a false `TS2536`/`TS2538`. Excluded:
    // - a naked type-parameter receiver (`object_placeholder`): unconstrained
    //   `T[K]` is a genuine error handled below;
    // - an *explicit* `unknown`/`any` keyword receiver (`unknown["x"]`): tsc does
    //   report `TS2339`/`TS2538` there, so it must not be suppressed.
    let object_is_explicit_top_keyword = matches!(
        object_type_for_placeholder.as_ref(),
        ParsedType::Unknown | ParsedType::UnknownKeyword | ParsedType::Any
    );
    if resolved_object.ty.is_unknown()
        && object_placeholder_name.is_none()
        && !object_is_explicit_top_keyword
    {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    if !index_is_valid_generic_key
        && (index_placeholder_name.is_some() || object_placeholder_name.is_some())
    {
        let index_name = index_placeholder_name
            .map(str::to_string)
            .unwrap_or_else(|| resolved_index.ty.name());
        let object_name = object_placeholder_name
            .map(str::to_string)
            .unwrap_or_else(|| resolved_object.ty.name());
        let mut diagnostic = Diagnostic::ts2536(&index_name, &object_name, ctx.file_name.clone());
        if let Some(span) = indexed_access
            .index_type
            .as_ref()
            .span()
            .or(indexed_access.span)
        {
            diagnostic = diagnostic.with_span(convert_span(span));
        }
        ctx.push(diagnostic);
        if generic_indexed_access {
            record_generic_indexed_access_invalid_key();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // `T[never]` is `never` in tsc — there is no key to select, so the access
    // contributes nothing. This is the empty-interface escape hatch
    // (`DO_NOT_USE_…[keyof DO_NOT_USE_…]` in React's `Key`/`ReactNode`), whose
    // arm has to vanish from the enclosing union instead of reporting TS2538.
    if matches!(resolved_index.ty, Type::Never) {
        return ResolvedType {
            ty: Type::Never,
            had_error: false,
        };
    }

    match (&resolved_object.ty, &resolved_index.ty) {
        (Type::Object(object_type), Type::StringLiteral(key)) => {
            if let Some(property_ty) = object_type.get_property_access_type(&key) {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: property_ty,
                    had_error: false,
                }
            } else {
                let mut diagnostic =
                    Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
                if let Some(span) = indexed_access.span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        // A numeric-literal index keys an object by its stringified value:
        // `{ 0: 1; 1: 0 }[0]` reads the `"0"` property. Object literals with
        // numeric keys are common in library-generated conditional-type tables
        // (e.g. Prisma's `{ 0: …; 1: … }[B]`).
        (Type::Object(object_type), Type::NumberLiteral(num)) => {
            if let Some(property_ty) = object_type.get_property_access_type(&num.value) {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: property_ty,
                    had_error: false,
                }
            } else {
                let mut diagnostic = Diagnostic::ts2339(
                    &num.value,
                    &resolved_object.ty.name(),
                    ctx.file_name.clone(),
                );
                if let Some(span) = indexed_access.span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        (Type::Object(object_type), Type::Union(union_ty)) => {
            let mut types = Vec::new();
            let mut had_error = false;
            for key_ty in union_ty.types() {
                let key = match key_ty {
                    Type::StringLiteral(key) => Some(key.clone()),
                    Type::NumberLiteral(num) => Some(num.value.clone()),
                    _ => None,
                };
                if let Some(key) = key {
                    if let Some(property_ty) = object_type.get_property_access_type(&key) {
                        types.push(property_ty);
                    } else {
                        let mut diagnostic = Diagnostic::ts2339(
                            &key,
                            &resolved_object.ty.name(),
                            ctx.file_name.clone(),
                        );
                        if let Some(span) = indexed_access.span {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }
                        ctx.push(diagnostic);
                        had_error = true;
                    }
                } else {
                    let mut diagnostic = Diagnostic::ts2538(&key_ty.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    had_error = true;
                }
            }

            if had_error {
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            } else {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: union_type(types),
                    had_error: false,
                }
            }
        }
        (Type::Tuple(elements), Type::NumberLiteral(num)) => {
            if let Ok(index) = num.value.parse::<usize>() {
                if let Some(element_ty) = elements.get(index) {
                    if generic_indexed_access {
                        record_generic_indexed_access_success();
                    }
                    ResolvedType {
                        ty: element_ty.clone(),
                        had_error: false,
                    }
                } else {
                    let mut diagnostic = Diagnostic::ts2493(
                        &resolved_object.ty.name(),
                        elements.len(),
                        index,
                        ctx.file_name.clone(),
                    );
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
                    }
                }
            } else {
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        (Type::Array(element_type), Type::Number) => ResolvedType {
            ty: {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                *element_type.clone()
            },
            had_error: false,
        },
        (Type::Tuple(elements), Type::Number) => {
            if generic_indexed_access {
                record_generic_indexed_access_success();
            }
            ResolvedType {
                ty: union_type(elements.clone()),
                had_error: false,
            }
        }
        (Type::Any, _) | (_, Type::Any) => {
            if generic_indexed_access {
                record_generic_indexed_access_success();
            }
            ResolvedType {
                ty: Type::Any,
                had_error: false,
            }
        }
        // Index a union receiver by a string key (`(A | B)["k"]`, notably
        // `T[number]["_zod"]`): read the key from each member and union the results.
        // Each member is peeled by `get_property_access_type`, so a member that is a
        // lazy/nominal reference (a deferred generic alias/interface instantiation)
        // resolves to its structural shape instead of being misreported as missing
        // the property. A member that degraded to `unknown`/`any` contributes that
        // (no cascade), matching how the receiver-errored branch selects properties.
        (Type::Union(union_ty), Type::StringLiteral(key)) => {
            let mut types = Vec::new();
            let mut missing = false;
            for member in union_ty.types() {
                if member.is_unknown() || matches!(member, Type::Any) {
                    types.push(member.clone());
                } else if let Some(property_ty) = member.get_property_access_type(key) {
                    types.push(property_ty);
                } else {
                    missing = true;
                    break;
                }
            }
            if missing {
                let mut diagnostic =
                    Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
                if let Some(span) = indexed_access.span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            } else {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: union_type(types),
                    had_error: false,
                }
            }
        }
        (_, Type::StringLiteral(key)) => {
            let mut diagnostic =
                Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
            if let Some(span) = indexed_access.span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);
            if generic_indexed_access {
                record_generic_indexed_access_unknown_fallback();
            }
            ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            }
        }
        (_, invalid_index) => {
            if let Type::Unknown = invalid_index {
                // In a generic context (a type-parameter receiver/key or a
                // substituted reference) an `unknown` index is a resolution
                // limitation we cannot validate — e.g. `T[keyof T]` where `keyof T`
                // could not be computed — not the literal `value[unknownKey]` that
                // tsc flags. Degrade silently rather than emit a false TS2538.
                if !generic_indexed_access
                    && ctx.options.diagnostic_profile != crate::context::DiagnosticProfile::Native
                {
                    let mut diagnostic =
                        Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
                if generic_indexed_access {
                    record_generic_indexed_access_unknown_fallback();
                }
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: !generic_indexed_access,
                };
            }
            let mut diagnostic = Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
            if let Some(span) = indexed_access.span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);
            if generic_indexed_access {
                record_generic_indexed_access_unknown_fallback();
            }
            ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            }
        }
    }
}
