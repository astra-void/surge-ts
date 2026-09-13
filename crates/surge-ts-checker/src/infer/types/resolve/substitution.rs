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
            let mut argument_had_error = false;
            let resolved_ty = if let Some(pre) = pre_resolved.and_then(|pre| pre.get(index)) {
                pre.clone()
            } else {
                let resolved_argument =
                    resolve_parsed_type(argument.clone(), ctx, resolving, parent_substitution);
                argument_had_error = resolved_argument.had_error;
                bound_had_error |= argument_had_error;
                resolved_argument.ty
            };

            if parsed_type_is_placeholder_reference(argument, parent_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), resolved_ty);
            } else {
                check_type_argument_constraint(
                    parameter,
                    type_parameters,
                    &resolved_ty,
                    argument_had_error,
                    name_span,
                    ctx,
                    resolving,
                    parent_substitution,
                    &substitution,
                    declaration_scope,
                );
                substitution.insert(parameter.name.clone(), resolved_ty);
            }
            if argument_had_error {
                substitution.mark_degraded(&parameter.name);
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
        if resolved_default.had_error {
            substitution.mark_degraded(&parameter.name);
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
    value_parameters: &[surge_ts_syntax::ParsedFunctionTypeParameter],
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

        // A signature's own type parameter is decided at the call site, so a
        // declared default only applies when nothing can infer it. Surge builds
        // this `FunctionType` without call context and drops the type-parameter
        // list, so a default baked into a *parameter* annotation hardens an
        // uninferred generic into a concrete type: `m<T = string>(x: T)` would
        // reject `o.m(42)`, and `f1<Q extends K[] = K[]>(o: Opts<Q>)` would
        // reject a caller's own `Opts<Q>`. Bind the uninferred sentinel there
        // instead — the reading assignability already treats as "could not be
        // inferred" — and keep the default for a parameter no argument mentions
        // (`m<T = string>(): T[]`), where tsc uses it too. Only a defaulted
        // parameter can be hardened, so the scan is gated on there being one.
        let inferable_from_arguments = parameter.default_type.is_some()
            && value_parameters.iter().any(|value_parameter| {
                parsed_type_mentions_type_parameter(&value_parameter.ty, &parameter.name)
            });

        let resolved = parameter.default_type.clone().map(|default_type| {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        });

        let ty = match resolved {
            Some(_) if inferable_from_arguments => Type::Unknown,
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

/// Whether `ty` names the type parameter `name` anywhere within it. A syntactic
/// over-approximation: a nested signature that shadows `name` still counts, which
/// only ever moves a binding toward the permissive uninferred sentinel.
fn parsed_type_mentions_type_parameter(ty: &ParsedType, name: &str) -> bool {
    match ty {
        ParsedType::Named(named) => {
            named.name == name
                || named
                    .type_arguments
                    .iter()
                    .any(|argument| parsed_type_mentions_type_parameter(argument, name))
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            parsed_type_mentions_type_parameter(inner, name)
        }
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => members
            .iter()
            .any(|member| parsed_type_mentions_type_parameter(member, name)),
        ParsedType::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| parsed_type_mentions_type_parameter(&parameter.ty, name))
                || parsed_type_mentions_type_parameter(&function.return_type, name)
        }
        ParsedType::Object(object) => {
            object
                .properties
                .iter()
                .any(|property| parsed_type_mentions_type_parameter(&property.ty, name))
                || object
                    .construct_signature
                    .as_deref()
                    .into_iter()
                    .chain(object.call_signature.as_deref())
                    .any(|signature| {
                        signature
                            .parameters
                            .iter()
                            .any(|parameter| {
                                parsed_type_mentions_type_parameter(&parameter.ty, name)
                            })
                            || parsed_type_mentions_type_parameter(&signature.return_type, name)
                    })
        }
        ParsedType::IndexedAccess(indexed) => {
            parsed_type_mentions_type_parameter(&indexed.object_type, name)
                || parsed_type_mentions_type_parameter(&indexed.index_type, name)
        }
        ParsedType::Mapped(mapped) => {
            parsed_type_mentions_type_parameter(&mapped.constraint, name)
                || parsed_type_mentions_type_parameter(&mapped.value_type, name)
        }
        ParsedType::Conditional(conditional) => {
            parsed_type_mentions_type_parameter(&conditional.check_type, name)
                || parsed_type_mentions_type_parameter(&conditional.extends_type, name)
                || parsed_type_mentions_type_parameter(&conditional.true_type, name)
                || parsed_type_mentions_type_parameter(&conditional.false_type, name)
        }
        ParsedType::TemplateLiteral(template) => template
            .interpolations
            .iter()
            .any(|part| parsed_type_mentions_type_parameter(part, name)),
        ParsedType::Predicate(predicate) => predicate
            .ty
            .as_ref()
            .is_some_and(|ty| parsed_type_mentions_type_parameter(ty, name)),
        _ => false,
    }
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

/// `Expect<false>` where `Expect<a extends true>`: a written type argument has to
/// satisfy its parameter's constraint, which surge never checked for a type
/// reference (only for a call's explicit `keyof` arguments). Both sides must be
/// settled — an argument or constraint that degraded says nothing about whether
/// the constraint holds — and a constraint naming an earlier parameter resolves
/// under the bindings made so far (`<T, K extends keyof T>`).
#[allow(clippy::too_many_arguments)]
fn check_type_argument_constraint(
    parameter: &ParsedTypeParameter,
    type_parameters: &[ParsedTypeParameter],
    argument: &Type,
    argument_had_error: bool,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    parent_substitution: &TypeParameterSubstitution,
    bound_so_far: &TypeParameterSubstitution,
    declaration_scope: Option<(
        &Option<std::sync::Arc<crate::symbols::TypeDeclarationScope>>,
        &str,
    )>,
) {
    let Some(constraint) = parameter.constraint.clone() else {
        return;
    };
    let constraint_for_display = constraint.clone();
    if argument_had_error || !constraint_judgeable(argument) {
        return;
    }
    // A constraint stated in terms of a *sibling* parameter (`K extends keyof T
    // & string`) is only as right as surge's model of that sibling. It resolves
    // to a literal union, which passes the judgeability test below while saying
    // nothing about whether the argument is really out of constraint: drizzle's
    // `PgSelectWithout<T, TDynamic, K extends keyof T & string>` reported its own
    // method-name union as violating `keyof T` in five files, because `T`'s keys
    // came back as an unrelated shape. The assertion idiom this check exists for
    // (`Expect<a extends true>`) never names a sibling, and `Pick`/`Omit` keep
    // their own dedicated check in `utility.rs`.
    if constraint_names_a_sibling(&constraint, type_parameters) {
        return;
    }

    let mut effective = parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
    effective.extend(bound_so_far.clone_with_reason(TypeCopyReason::SubstitutionChanged));
    // A constraint is authored in the *declaring* module and names its siblings
    // bare, exactly like a default: resolving it under the consumer's scope
    // reports every such sibling as an unknown name. The resolution is also
    // speculative — it exists only to judge the argument — so anything it emits
    // on the way is rolled back.
    let diagnostics_before = ctx.diagnostics().len();
    let resolved_constraint = if let Some((scope, file_name)) = declaration_scope {
        with_type_declaration_scope(scope, ctx, |ctx| {
            with_file_name(ctx, file_name, |ctx| {
                resolve_parsed_type(constraint, ctx, resolving, &effective)
            })
        })
    } else {
        resolve_parsed_type(constraint, ctx, resolving, &effective)
    };
    ctx.truncate_diagnostics_releasing_utility_keys(diagnostics_before);
    if resolved_constraint.had_error || !constraint_judgeable(&resolved_constraint.ty) {
        return;
    }

    if !surge_ts_types::is_assignable_to(argument, &resolved_constraint.ty) {
        let constraint_name = written_constraint_display(&constraint_for_display, &effective)
            .unwrap_or_else(|| resolved_constraint.ty.name().to_string());
        crate::infer::types::diagnostics::emit_type_argument_constraint(
            argument,
            &constraint_name,
            name_span,
            ctx,
        );
    }
}

/// The constraint as written, for the diagnostic. `keyof T` is spelled out
/// rather than expanded because that is how tsc names it and how `Pick`'s own
/// check in `utility.rs` already reports it — two spellings of one constraint
/// are two dedup keys, and the same violation was reported twice. Kept local
/// rather than added to `parsed_type_display`, whose output also prefixes
/// rendered function types.
fn written_constraint_display(
    constraint: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> Option<String> {
    match constraint {
        ParsedType::KeyOf(inner) => Some(format!(
            "keyof {}",
            written_constraint_display(inner, substitution)?
        )),
        // A constraint names the declaration's *own* parameters (`Pick<T, K
        // extends keyof T>`), and tsc renders them substituted — `keyof User`,
        // not `keyof T`.
        ParsedType::Named(named) if named.type_arguments.is_empty() => Some(
            substitution
                .get(&named.name)
                .map(|bound| bound.name().to_string())
                .unwrap_or_else(|| named.name.clone()),
        ),
        other => crate::driver::parsed_type_display(other),
    }
}

/// Whether `constraint` mentions one of the declaration's own type parameters.
/// Anything this cannot look inside counts as mentioning one: the check is worth
/// having only where it is certain, and a missed report costs less than a false
/// one.
fn constraint_names_a_sibling(constraint: &ParsedType, siblings: &[ParsedTypeParameter]) -> bool {
    let names_sibling = |name: &str| siblings.iter().any(|sibling| sibling.name == name);
    match constraint {
        ParsedType::String
        | ParsedType::Number
        | ParsedType::Boolean
        | ParsedType::BigInt
        | ParsedType::Symbol
        | ParsedType::Undefined
        | ParsedType::Void
        | ParsedType::Any
        | ParsedType::Unknown
        | ParsedType::UnknownKeyword
        | ParsedType::Never
        | ParsedType::StringLiteral(_)
        | ParsedType::NumberLiteral(_)
        | ParsedType::BooleanLiteral(_) => false,
        ParsedType::Named(named) => {
            names_sibling(&named.name)
                || named
                    .type_arguments
                    .iter()
                    .any(|argument| constraint_names_a_sibling(argument, siblings))
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            constraint_names_a_sibling(inner, siblings)
        }
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => members
            .iter()
            .any(|member| constraint_names_a_sibling(member, siblings)),
        _ => true,
    }
}

/// A type whose constraint relationship surge can judge without leaning on
/// structural assignability. Deliberately only primitives and literals: every
/// structural gap surge still has (an interface that should satisfy `object`, a
/// lazy reference whose shape is not forced) would otherwise surface here as a
/// false `TS2344` on code that is fine, and this check has no way to tell that
/// from a real violation. The assertion idiom this exists for — `Expect<a extends
/// true>`, `K extends keyof T` with a written key — is entirely in this domain.
fn constraint_judgeable(ty: &Type) -> bool {
    match ty {
        Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(constraint_judgeable),
        _ => false,
    }
}
