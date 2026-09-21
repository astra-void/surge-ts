//! Function/arrow signature mapping, parameter binding, and signature registration.

use super::*;

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedArrayBindingPattern, ParsedBindingName, ParsedFunctionBodyStatement,
    ParsedFunctionParameter, ParsedObjectBindingElement, ParsedObjectBindingPattern, ParsedType,
    ParsedTypeParameter, TextSpan,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use crate::checks::expr::evaluate_expression;
use crate::checks::var::widen_implicit_variable_initializer_type;
use crate::context::convert_span;
use crate::context::{CheckerContext, FileKind};
use crate::flow::{FunctionFlowState, analyze_function_body_flow, collect_function_flow_facts};
use crate::infer::{
    InferredExpression, TypeParameterSubstitution, map_parsed_type_with_substitution,
    report_duplicate_type_parameters,
};
use crate::metrics::alloc_function_type;
use crate::symbols::{FunctionSignatureInfo, ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

/// `SURGE_TSC_IMPLICIT_ANY=1`: report implicit-any exactly where tsc does,
/// dropping surge's two suppression depths.
///
/// tsc's own exemptions are three — `reportErrors` off, a private ambient
/// member, and a JS file without `checkJS` (`getTypeForVariableLikeDeclaration`
/// tail, then `reportImplicitAny`). It has **no** "the contextual type was
/// degraded" rule, so `unmodelled_jsx_props_depth` and
/// `degraded_expected_type_depth` are surge's own.
///
/// They are not a wrong rule to delete, though: they stand in for the
/// contextual types surge fails to supply where tsc has one. Measured
/// 2026-09-16 with them off — trpc FN 50 -> 37, but FP trpc 7 -> 724,
/// zod 0 -> 357, tanstack 7 -> 211. The dominant zod shape is
/// `core.$constructor("…", (inst, def) => …)` assigned to a
/// `core.$constructor<$ZodCheckLessThan>` annotation: the callback parameters
/// get no contextual type because that very interface resolves to
/// `Object{1 props: init}` with **no construct signature** — `new (def: D): T`
/// is dropped — so the contextual type is already half missing before
/// inference runs. Close that, then turn this on.
fn tsc_implicit_any_rule() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_TSC_IMPLICIT_ANY").as_deref() == Ok("1"))
}

pub(crate) fn emit_parameter_diagnostics(
    parameter: &ParsedFunctionParameter,
    contextual_type: Option<&Type>,
    ctx: &mut CheckerContext,
) {
    // tsc's own exemptions are three — `reportErrors` off, a private ambient
    // member, a JS file without `checkJS` (`getTypeForVariableLikeDeclaration`
    // tail). It has no "the contextual type was degraded" rule, so these two
    // depths are surge's, and they are load-bearing rather than wrong:
    // dropping them to match tsc exactly closes 13 tRPC false negatives and
    // opens ~1,300 false positives (trpc 7 -> 724, zod 0 -> 357, tanstack
    // 7 -> 211, measured 2026-09-16). They stand in for the contextual types
    // surge fails to supply where tsc has one; removing them is only correct
    // once that gap is closed.
    if !ctx.options.no_implicit_any
        || parameter.declared_type.is_some()
        || parameter.initializer.is_some()
        || (!tsc_implicit_any_rule()
            && (ctx.unmodelled_jsx_props_depth > 0 || ctx.degraded_expected_type_depth > 0))
    {
        return;
    }

    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            if contextual_type.is_some() {
                return;
            }
            let diagnostic = Diagnostic::ts7006(name, ctx.file_name.clone());
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(*span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            if contextual_type.is_some() {
                return;
            }
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            if contextual_type.is_some() {
                return;
            }
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn emit_array_binding_pattern_diagnostics(
    pattern: &ParsedArrayBindingPattern,
    ctx: &mut CheckerContext,
) {
    for element in pattern.elements.iter().flatten() {
        emit_array_binding_element_diagnostic(element, ctx);
    }
}

fn emit_array_binding_element_diagnostic(
    binding_name: &ParsedBindingName,
    ctx: &mut CheckerContext,
) {
    match binding_name {
        ParsedBindingName::Identifier { name, span } => {
            let diagnostic = Diagnostic::ts7031(name, "any", ctx.file_name.clone());
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(*span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn emit_object_binding_pattern_diagnostics(
    pattern: &ParsedObjectBindingPattern,
    ctx: &mut CheckerContext,
) {
    for element in &pattern.elements {
        emit_object_binding_element_diagnostic(element, ctx);
    }
    // The `...rest` binding gets an (empty) object type, not implicit `any`, so
    // tsc emits no TS7031 for it even when the surrounding pattern is untyped.
}

pub(crate) fn emit_object_binding_element_diagnostic(
    element: &ParsedObjectBindingElement,
    ctx: &mut CheckerContext,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            let diagnostic = Diagnostic::ts7031(name, "any", ctx.file_name.clone());
            let span = (*span).or(element.name_span);
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn parameter_identifier_name(parameter: &ParsedFunctionParameter) -> Option<&str> {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(crate) fn parameter_scope_type(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
) -> Type {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { .. } => {
            let ty =
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone());
            // An optional parameter (`x?: T`) is `T | undefined` inside the body,
            // so comparing it to `undefined` is intentional and `=> undefined`
            // flows through narrowing. A defaulted parameter (`x: T = …`) is not
            // optional in the body (the default fills the gap), and a rest
            // parameter is already an array, so neither widens. This only affects
            // the in-body view; the signature's parameter type (used to check
            // call arguments) is unchanged.
            if parameter.optional && parameter.initializer.is_none() && !parameter.rest {
                surge_ts_types::union_type(vec![ty, Type::Undefined])
            } else {
                ty
            }
        }
        // A destructured parameter's elements read the parameter's type; the
        // element lookups stay permissive on a miss.
        ParsedBindingName::ObjectPattern(_) | ParsedBindingName::ArrayPattern(_) => {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone())
        }
        ParsedBindingName::Unsupported { .. } => Type::Any,
    }
}

pub(crate) fn insert_binding_name(
    binding_name: &ParsedBindingName,
    ty: Type,
    scopes: &mut ScopeStack,
) {
    match binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.as_str(),
                SymbolInfo {
                    ty,
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, ty, scopes);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            insert_array_binding_pattern_bindings(pattern, ty, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

/// The element type a destructuring position reads: the tuple element at `index`
/// for a tuple source, the element type for an array source, and `any`/`unknown`
/// otherwise (conservative — keeps the binding in scope without cascading).
fn array_binding_element_type(source: &Type, index: usize) -> Type {
    match source {
        Type::Tuple(elements) => elements.get(index).cloned().unwrap_or(Type::Undefined),
        Type::Array(element) => (**element).clone(),
        Type::Any => Type::Any,
        _ => Type::Any,
    }
}

pub(crate) fn insert_array_binding_pattern_bindings(
    pattern: &ParsedArrayBindingPattern,
    source_type: Type,
    scopes: &mut ScopeStack,
) {
    for (index, element) in pattern.elements.iter().enumerate() {
        if let Some(element) = element {
            let element_type = array_binding_element_type(&source_type, index);
            insert_binding_name(element, element_type, scopes);
        }
    }
    if let Some(rest) = &pattern.rest {
        // `[a, ...rest]` binds `rest` to the remaining elements. We model it as
        // an array of the source element type (or the source as-is) — precise
        // enough to keep `rest` usable without an exact `slice` shape.
        let rest_type = match &source_type {
            Type::Array(_) => source_type.clone(),
            Type::Tuple(elements) => {
                Type::Array(Box::new(elements.last().cloned().unwrap_or(Type::Any)))
            }
            _ => Type::Any,
        };
        insert_binding_name(rest, rest_type, scopes);
    }
}

/// Evaluates the defaults a destructuring pattern writes (`{ c = fallback }`)
/// in the scope the pattern binds into, for their own diagnostics.
pub(crate) fn check_binding_pattern_defaults(
    binding: &ParsedBindingName,
    bound_type: Option<&Type>,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    match binding {
        ParsedBindingName::ObjectPattern(pattern) => {
            for element in &pattern.elements {
                // The destructured member's own type is the default's
                // contextual type, which is what types an arrow default's
                // parameters (`{ reducer = (items, chunk) => … }`).
                let member_type = bound_type
                    .map(Type::peeled)
                    .and_then(|ty| ty.get_property_access_type(&element.property_name))
                    .map(|ty| surge_ts_types::remove_nullish(&ty));
                if let Some(default) = element.default_value.as_deref() {
                    let (expression, span) = default;
                    let _ = crate::checks::expected::evaluate_expression_with_expected_type(
                        expression,
                        *span,
                        member_type.as_ref(),
                        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
                        scopes.visible_symbols(),
                        ctx,
                    );
                }
                check_binding_pattern_defaults(
                    &element.binding_name,
                    member_type.as_ref(),
                    scopes,
                    ctx,
                );
            }
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                check_binding_pattern_defaults(element, None, scopes, ctx);
            }
        }
        ParsedBindingName::Identifier { .. } | ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn insert_parameter_bindings(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
    scopes: &mut ScopeStack,
) {
    insert_binding_name(
        &parameter.binding_name,
        parameter_scope_type(parameter, parameter_type),
        scopes,
    );
}

pub(crate) fn insert_object_binding_pattern_bindings(
    pattern: &ParsedObjectBindingPattern,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    for element in &pattern.elements {
        let mut element_type = object_binding_element_type(&parameter_type, &element.property_name);
        // `const { numRefs = 0 } = params` binds the default when the property is
        // absent, so the binding is never `undefined`.
        if element.has_default {
            element_type = surge_ts_types::remove_undefined(&element_type);
        }
        insert_object_binding_element_binding(element, element_type, scopes);
    }
    // `{ a, ...rest }` binds `rest` to the remaining properties. The exact
    // `Omit<T, ...>` shape is not modelled; binding it to the source type keeps
    // `rest` in scope (and spreadable via `{...rest}`) without a TS2304 cascade.
    if let Some(rest) = &pattern.rest {
        insert_binding_name(
            rest,
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone()),
            scopes,
        );
    }
}

/// The type a `{ property }` destructuring position reads. A miss keeps the
/// binding permissive rather than handing it the *source* type, which made every
/// use of a destructured binding compare against the whole object
/// (`for (const { schema } of items) schema.safeParse(…)`).
fn object_binding_element_type(source: &Type, property_name: &str) -> Type {
    match source {
        Type::Any => Type::Any,
        source if source.is_unknown() => {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || source.clone())
        }
        // `Type::get_property_access_type` does not distribute over a union, and
        // an array of object literals is exactly that.
        Type::Union(union) => {
            let members: Option<Vec<Type>> = union
                .types()
                .iter()
                .map(|member| member.get_property_access_type(property_name))
                .collect();
            match members {
                Some(members) => surge_ts_types::union_type(members),
                None => Type::Any,
            }
        }
        source => source
            .get_property_access_type(property_name)
            .unwrap_or(Type::Any),
    }
}

pub(crate) fn insert_object_binding_element_binding(
    element: &ParsedObjectBindingElement,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.as_str(),
                SymbolInfo {
                    ty: parameter_type,
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, parameter_type, scopes);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            insert_array_binding_pattern_bindings(pattern, parameter_type, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn map_function_signature(
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    contextual_parameter_types: Option<&[Type]>,
    ctx: &mut CheckerContext,
) -> FunctionType {
    report_duplicate_type_parameters(type_parameters, ctx);

    // Register the signature's type parameters (with their constraints) for the
    // duration of parameter/return-type resolution. The placeholder substitution
    // alone marks `K` as generic, but the *constraint* (`K extends keyof Hooks`)
    // lives only in this scope; without it a constrained indexed access in the
    // return type (`Required<Hooks>[K]`) cannot be recognised as a valid generic
    // index and degrades to a false `TS2536`.
    let pushed_type_parameter_scope = !type_parameters.is_empty();
    if pushed_type_parameter_scope {
        ctx.push_type_parameter_scope(type_parameters, None);
    }

    let type_parameter_substitution = build_type_parameter_substitution(type_parameters);
    let mut parameter_types = Vec::with_capacity(parameters.len());
    let mut parameter_symbols = None;
    let mut parameter_bindings: Vec<(String, Type)> = Vec::new();

    let outer_parameter_bindings = std::mem::take(&mut ctx.signature_parameter_bindings);
    for (index, parameter) in parameters.iter().enumerate() {
        let inferred_parameter_type = if let Some(declared_type) = parameter.declared_type.clone() {
            ctx.signature_parameter_bindings = parameter_bindings.clone();
            let mapped =
                map_parsed_type_with_substitution(declared_type, ctx, &type_parameter_substitution);
            ctx.signature_parameter_bindings.clear();
            mapped
        } else if let Some(initializer) = parameter.initializer.as_ref() {
            let parameter_symbols = parameter_symbols.get_or_insert_with(|| {
                let mut symbols = ctx
                    .symbols
                    .clone_with_reason(TypeCopyReason::FunctionBodySetup);
                for (name, ty) in &parameter_bindings {
                    let _ = symbols.insert(
                        name.clone(),
                        SymbolInfo {
                            ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                                ty.clone()
                            }),
                            kind: SymbolKind::Parameter,
                            function_signature: None,
                        },
                    );
                }
                symbols
            });
            let inferred_initializer = evaluate_expression(
                initializer,
                parameter.initializer_span,
                parameter_symbols,
                ctx,
            );

            match inferred_initializer {
                InferredExpression::Known(ty) => {
                    widen_implicit_variable_initializer_type(SymbolKind::Let, initializer, &ty, false)
                }
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => Type::Unknown,
            }
        } else {
            Type::Any
        };

        if let Some(name) = parameter_identifier_name(parameter) {
            let parameter_binding_type =
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                    inferred_parameter_type.clone()
                });
            if let Some(parameter_symbols) = parameter_symbols.as_mut() {
                let _ = parameter_symbols.insert(
                    name.to_string(),
                    SymbolInfo {
                        ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                            parameter_binding_type.clone()
                        }),
                        kind: SymbolKind::Parameter,
                        function_signature: None,
                    },
                );
            }
            parameter_bindings.push((name.to_string(), parameter_binding_type));
        }

        parameter_types.push(inferred_parameter_type);

        if ctx.options.no_implicit_any {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
        }
    }

    ctx.signature_parameter_bindings = parameter_bindings.clone();
    let function_return_type = return_type
        .map(|return_type| {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                map_parsed_type_with_substitution(
                    return_type.clone(),
                    ctx,
                    &type_parameter_substitution,
                )
            })
        })
        .unwrap_or(Type::Unknown);
    ctx.signature_parameter_bindings = outer_parameter_bindings;

    if pushed_type_parameter_scope {
        ctx.pop_type_parameter_scope();
    }

    alloc_function_type(
        parameter_types,
        function_return_type,
        parameters.last().is_some_and(|parameter| parameter.rest),
        required_parameter_count(parameters),
    )
    .with_parameter_names(written_binding_names(parameters))
    .with_type_parameter_head(type_parameter_head(type_parameters))
}

/// Renders a signature's type-parameter list the way tsc prefixes it
/// (`T extends FieldBag = FieldBag`), without the angle brackets. Syntactic, so
/// it neither resolves nor caches anything; `None` when a constraint or default
/// is not renderable, which keeps the un-prefixed form rather than a partial one.
pub(crate) fn type_parameter_head(parameters: &[ParsedTypeParameter]) -> Option<String> {
    if parameters.is_empty() {
        return None;
    }
    let mut rendered = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let mut text = parameter.name.clone();
        if let Some(constraint) = parameter.constraint.as_ref() {
            text.push_str(" extends ");
            text.push_str(&crate::driver::parsed_type_display(constraint)?);
        }
        if let Some(default_type) = parameter.default_type.as_ref() {
            text.push_str(" = ");
            text.push_str(&crate::driver::parsed_type_display(default_type)?);
        }
        rendered.push(text);
    }
    Some(rendered.join(", "))
}

/// The names as written, for display only (see
/// [`surge_ts_types::FunctionType::with_parameter_names`]). A destructured
/// parameter has no written name, so it keeps the bare type rendering.
pub(crate) fn written_binding_names(
    parameters: &[ParsedFunctionParameter],
) -> Vec<Option<Arc<str>>> {
    parameters
        .iter()
        .map(|parameter| match &parameter.binding_name {
            ParsedBindingName::Identifier { name, .. } => Some(Arc::<str>::from(name.as_str())),
            _ => None,
        })
        .collect()
}

pub(crate) fn map_lazy_dependency_function_signature(
    function: &surge_ts_syntax::ParsedFunctionDeclaration,
    ctx: &mut CheckerContext,
) -> FunctionType {
    report_duplicate_type_parameters(&function.type_parameters, ctx);
    crate::program::record_program_counter(|c| {
        c.lazy_signature_create_count += 1;
        c.lazy_signature_generic_annotation_create_count += function
            .type_parameters
            .iter()
            .map(|parameter| {
                u64::from(parameter.constraint.is_some())
                    + u64::from(parameter.default_type.is_some())
            })
            .sum::<u64>();
    });

    let pushed_type_parameter_scope = !function.type_parameters.is_empty();
    if pushed_type_parameter_scope {
        ctx.push_type_parameter_scope(&function.type_parameters, None);
    }
    let signature_environment =
        crate::infer::LazySignatureEnvironment::new(&function.type_parameters);
    let type_parameter_substitution = build_type_parameter_substitution(&function.type_parameters);

    let declaration_start = function.name_span.map_or(0, |span| span.start);
    let parameter_types = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let Some(annotation) = parameter.declared_type.clone() else {
                return Type::Any;
            };
            if !defer_dependency_signature_annotation(&annotation) {
                return map_parsed_type_with_substitution(
                    annotation,
                    ctx,
                    &type_parameter_substitution,
                );
            }
            let is_this = parameter_identifier_name(parameter) == Some("this");
            crate::infer::make_lazy_signature_annotation_reference(
                ctx,
                &function.name,
                declaration_start,
                if is_this {
                    crate::infer::LazySignatureComponent::ThisParameter
                } else {
                    crate::infer::LazySignatureComponent::Parameter(index)
                },
                annotation,
                signature_environment.clone(),
            )
        })
        .collect();
    let return_type = function
        .return_type
        .clone()
        .map_or(Type::Unknown, |annotation| {
            if !defer_dependency_signature_annotation(&annotation) {
                return map_parsed_type_with_substitution(
                    annotation,
                    ctx,
                    &type_parameter_substitution,
                );
            }
            crate::infer::make_lazy_signature_annotation_reference(
                ctx,
                &function.name,
                declaration_start,
                crate::infer::LazySignatureComponent::Return,
                annotation,
                signature_environment,
            )
        });

    if pushed_type_parameter_scope {
        ctx.pop_type_parameter_scope();
    }

    alloc_function_type(
        parameter_types,
        return_type,
        function
            .parameters
            .last()
            .is_some_and(|parameter| parameter.rest),
        required_parameter_count(&function.parameters),
    )
    .with_parameter_names(written_binding_names(&function.parameters))
    .with_type_parameter_head(type_parameter_head(&function.type_parameters))
}

fn defer_dependency_signature_annotation(annotation: &ParsedType) -> bool {
    match annotation {
        ParsedType::Object(_)
        | ParsedType::Tuple(_)
        | ParsedType::VariadicTuple(_)
        | ParsedType::Readonly(_)
        | ParsedType::Union(_)
        | ParsedType::Intersection(_)
        | ParsedType::Function(_)
        | ParsedType::TypeOf(_)
        | ParsedType::KeyOf(_)
        | ParsedType::IndexedAccess(_)
        | ParsedType::Mapped(_)
        | ParsedType::Conditional(_)
        | ParsedType::TemplateLiteral(_) => true,
        ParsedType::Array(element) => defer_dependency_signature_annotation(element),
        ParsedType::String
        | ParsedType::Number
        | ParsedType::Boolean
        | ParsedType::BigInt
        | ParsedType::Symbol
        | ParsedType::Undefined
        | ParsedType::Void
        | ParsedType::Any
        | ParsedType::ErrorType
        | ParsedType::Unknown
        | ParsedType::UnknownKeyword
        | ParsedType::Never
        | ParsedType::StringLiteral(_)
        | ParsedType::NumberLiteral(_)
        | ParsedType::BooleanLiteral(_)
        | ParsedType::Named(_)
        | ParsedType::Infer(_)
        | ParsedType::Predicate(_)
        | ParsedType::InferredMember(_) => false,
    }
}

pub(crate) fn required_parameter_count(parameters: &[ParsedFunctionParameter]) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.initializer.is_some() || parameter.rest {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

pub(crate) fn has_contextual_unknown_object_binding_pattern(
    parameters: &[ParsedFunctionParameter],
    contextual_parameter_types: Option<&[Type]>,
) -> bool {
    let Some(contextual_parameter_types) = contextual_parameter_types else {
        return false;
    };

    parameters.iter().enumerate().any(|(index, parameter)| {
        matches!(parameter.binding_name, ParsedBindingName::ObjectPattern(_))
            && parameter.declared_type.is_none()
            // Only a written `unknown` is an error in tsc. The degradation
            // sentinel means the contextual type failed to resolve, and
            // reporting it turns every unresolved callback slot into a false
            // positive (zod `superRefine`, `refine`).
            && contextual_parameter_types
                .get(index)
                .is_some_and(|ty| matches!(ty, Type::GenuineUnknown))
    })
}

pub(crate) fn build_type_parameter_substitution(
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();

    for type_parameter in type_parameters {
        substitution.insert_placeholder(
            type_parameter.name.clone(),
            Type::type_parameter(&type_parameter.name),
        );
    }

    substitution
}

pub(crate) fn function_signature_info(
    type_parameters: &[ParsedTypeParameter],
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    declaring_file: &str,
) -> Arc<FunctionSignatureInfo> {
    Arc::new(FunctionSignatureInfo {
        overloaded: false,
        type_parameters: type_parameters.to_vec(),
        parameter_types: parameters
            .iter()
            .map(|parameter| parameter.declared_type.clone())
            .collect(),
        parameter_names: parameters
            .iter()
            .map(|parameter| match &parameter.binding_name {
                surge_ts_syntax::ParsedBindingName::Identifier { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect(),
        rest: parameters.last().is_some_and(|parameter| parameter.rest),
        return_type: return_type.cloned(),
        declaring_file: Some(Arc::from(declaring_file)),
        namespace_prefix: None,
        predicate_overload: None,
        overload_alternatives: Vec::new(),
    })
}

/// [`function_signature_info`] for a value whose *annotation* is a generic
/// function type (`declare const f: <T>() => Box<T>`, or an arrow assigned to an
/// annotated binding). Without it a call supplying explicit type arguments had
/// nothing to re-resolve the return annotation against, so the result kept the
/// uninstantiated `Box<T>` and degraded — vitest's
/// `__getSpy<OnError>()` then read as not callable.
pub(crate) fn function_type_signature_info(
    function_type: &surge_ts_syntax::ParsedFunctionType,
    declaring_file: &str,
) -> Arc<FunctionSignatureInfo> {
    let value_parameters = function_type
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_this);
    Arc::new(FunctionSignatureInfo {
        overloaded: false,
        type_parameters: function_type.type_parameters.clone(),
        parameter_types: value_parameters
            .clone()
            .map(|parameter| Some(parameter.ty.clone()))
            .collect(),
        parameter_names: value_parameters
            .map(|parameter| parameter.name.clone())
            .collect(),
        rest: function_type
            .parameters
            .last()
            .is_some_and(|parameter| parameter.rest),
        return_type: Some((*function_type.return_type).clone()),
        declaring_file: Some(Arc::from(declaring_file)),
        namespace_prefix: None,
        predicate_overload: None,
        overload_alternatives: Vec::new(),
    })
}

/// [`function_signature_info`] for a member published under a qualified
/// `ns.member` key. Instantiation re-resolves the written annotations, whose
/// bare sibling names (`Dispatch` inside `React.useState`) only resolve under
/// the namespace prefix.
pub(crate) fn namespace_member_signature_info(
    type_parameters: &[ParsedTypeParameter],
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    declaring_file: &str,
    namespace_prefix: &str,
) -> Arc<FunctionSignatureInfo> {
    let base = function_signature_info(type_parameters, parameters, return_type, declaring_file);
    let mut info = (*base).clone();
    info.namespace_prefix = Some(Arc::from(namespace_prefix));
    Arc::new(info)
}

/// Resolves every written type parameter constraint and default (`<T extends
/// C = D>`) under the declaration's own type parameters, so a name either
/// refers to is reported like any other type reference.
pub(crate) fn check_type_parameter_declarations(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    if type_parameters
        .iter()
        .all(|parameter| parameter.constraint.is_none() && parameter.default_type.is_none())
    {
        return;
    }
    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        for parameter in type_parameters {
            for written in [&parameter.constraint, &parameter.default_type].into_iter().flatten() {
                let _ = crate::infer::map_parsed_type(written.clone(), ctx);
            }
        }
    });
}

pub(crate) fn with_type_parameter_scope<R>(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let mut scope = std::collections::HashMap::new();
    for type_parameter in type_parameters {
        scope.insert(type_parameter.name.clone(), Type::Unknown);
    }

    ctx.push_type_parameter_scope(type_parameters, Some(scope));
    let result = f(ctx);
    ctx.pop_type_parameter_scope();
    result
}

/// Folds two signatures of one overload group into a single callable shape: a
/// position declared differently across overloads becomes the union of what the
/// overloads accept, the arity floor drops to the smallest, and a return type
/// that differs between overloads widens to `any` (which overload applies depends
/// on the arguments, which one signature cannot express).
///
/// The union — rather than `any` — keeps the group's contextual typing usable, so
/// an object-literal argument still types its callback parameters.
pub(crate) fn merge_overload_group_signatures(a: &FunctionType, b: &FunctionType) -> FunctionType {
    let (longer, shorter) = if a.parameters().len() >= b.parameters().len() {
        (a, b)
    } else {
        (b, a)
    };

    let parameters = longer
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, ty)| match shorter.parameters().get(index) {
            Some(other) if other == ty => ty.clone(),
            Some(other) => surge_ts_types::union_type(vec![ty.clone(), other.clone()]),
            None => ty.clone(),
        })
        .collect::<Vec<_>>();

    let return_type = if a.return_type() == b.return_type() {
        a.return_type().clone()
    } else {
        Type::Any
    };

    // The fold is the *shape* a call is checked against; the members are what
    // decide its return type once the arguments are known. `a` is the group so
    // far and `b` the newest declaration, so the list stays in declaration order.
    let mut members = Vec::with_capacity(2);
    a.push_overload_members(&mut members);
    b.push_overload_members(&mut members);

    alloc_function_type(
        parameters,
        return_type,
        a.is_variadic() || b.is_variadic(),
        a.required_parameter_count()
            .min(b.required_parameter_count()),
    )
    .with_overloads(members)
}

/// Whether a signature's return is a `x is T` type predicate (an `asserts`
/// clause is a different narrowing and is not one).
fn declares_type_predicate(signature: &FunctionSignatureInfo) -> bool {
    matches!(
        &signature.return_type,
        Some(surge_ts_syntax::ParsedType::Predicate(predicate))
            if !predicate.asserts && predicate.ty.is_some()
    )
}

/// Records the group's predicate-bearing overload on the signature the group
/// keeps, so guard narrowing can reach it without the folded signature changing.
/// The first such overload wins, and a signature that already declares a
/// predicate itself needs nothing.
fn attach_predicate_overload(
    kept: Option<Arc<FunctionSignatureInfo>>,
    incoming: Option<&Arc<FunctionSignatureInfo>>,
) -> Option<Arc<FunctionSignatureInfo>> {
    let kept = kept?;
    let Some(incoming) = incoming else {
        return Some(kept);
    };
    if kept.predicate_overload.is_some()
        || declares_type_predicate(&kept)
        || !declares_type_predicate(incoming)
    {
        return Some(kept);
    }
    let mut carried = (*kept).clone();
    carried.predicate_overload = Some(incoming.clone());
    Some(Arc::new(carried))
}

/// Records a later overload of the group on the signature the group keeps, so a
/// generic call can re-resolve every overload's parameter annotations instead of
/// only the first's. See `FunctionSignatureInfo::overload_alternatives`.
///
/// Bounded: a group declaring more overloads than this contributes only its
/// first few, since every one folded in costs an inference run per call.
fn attach_overload_alternative(
    kept: Option<Arc<FunctionSignatureInfo>>,
    incoming: Option<&Arc<FunctionSignatureInfo>>,
) -> Option<Arc<FunctionSignatureInfo>> {
    const MAX_ALTERNATIVES: usize = 6;

    let kept = kept?;
    let incoming = incoming?;
    if kept.overload_alternatives.len() >= MAX_ALTERNATIVES {
        return Some(kept);
    }
    let mut carried = (*kept).clone();
    carried.overload_alternatives.push(Arc::clone(incoming));
    Some(Arc::new(carried))
}

/// Flags the template signature of an overload group; see
/// `FunctionSignatureInfo::overloaded`.
pub(crate) fn mark_overloaded(
    signature: Option<Arc<FunctionSignatureInfo>>,
) -> Option<Arc<FunctionSignatureInfo>> {
    signature.map(|signature| {
        if signature.overloaded {
            return signature;
        }
        let mut flagged = (*signature).clone();
        flagged.overloaded = true;
        Arc::new(flagged)
    })
}

pub(crate) fn register_function_signature(
    name: String,
    function_type: FunctionType,
    function_signature: Option<Arc<FunctionSignatureInfo>>,
    symbols: &mut SymbolTable,
    replace_existing: bool,
    is_implementation: bool,
    // The implementation of a function this file also declares overloads for.
    // tsc resolves calls against the overloads alone, so it stays out of the
    // group: folded in, its usual `any` parameters accept every argument.
    hidden_behind_overloads: bool,
) -> bool {
    let symbol_exists = matches!(
        symbols.get(&name),
        Some(existing) if matches!(existing.kind, SymbolKind::Function)
    );
    if symbol_exists && !replace_existing {
        crate::program::record_program_counter(|c| c.overload_group_create_count += 1);
    }

    // TS2393 ("Duplicate function implementation") fires only when *this*
    // declaration has a body and another implementation was already registered.
    // Bodyless declarations (overload signatures, ambient `declare function`s)
    // merge as overloads, so two of them — or an overload preceding an
    // implementation — is not a duplicate.
    let duplicate_implementation = is_implementation && symbols.has_function_implementation(&name);
    if is_implementation {
        symbols.mark_function_implementation(&name);
    }

    if symbol_exists && !replace_existing {
        // Overload group: fold the incoming signature into the one already
        // registered instead of keeping only the first. Positions that differ
        // across overloads widen to `any` and the arity floor drops to the
        // smallest — the same permissive merge interface methods and type-literal
        // call signatures already use. Without it, a later overload's call
        // (`cacheLife("minutes")` against a first overload declared `'default'`)
        // is a false TS2345.
        // A second *implementation* is a duplicate declaration (TS2393), not an
        // overload group: the first signature stays authoritative for calls.
        if !duplicate_implementation
            && !hidden_behind_overloads
            && let Some(existing) = symbols.get(&name)
            && let Type::Function(existing_function) = &existing.ty
        {
            let merged = merge_overload_group_signatures(existing_function, &function_type);
            let existing_signature = existing.function_signature.clone();
            // The group keeps one declaration's parsed signature, and swapping
            // which one would break the other overloads' callers. But a type
            // predicate is the one thing only the declaration that wrote it can
            // supply — guard narrowing reads it off this signature — so the
            // predicate-bearing overload is carried alongside instead.
            // ts-pattern's `isMatching` declares its predicate on the *second*
            // overload, and without this the guard found none and narrowed
            // nothing.
            let existing_signature =
                attach_predicate_overload(existing_signature, function_signature.as_ref());
            let existing_signature =
                attach_overload_alternative(existing_signature, function_signature.as_ref());
            symbols.insert(
                name,
                SymbolInfo {
                    ty: Type::Function(merged),
                    kind: SymbolKind::Function,
                    function_signature: mark_overloaded(existing_signature.or(function_signature)),
                },
            );
        }
        return duplicate_implementation;
    }

    if !symbol_exists || replace_existing {
        symbols.insert(
            name,
            SymbolInfo {
                ty: Type::Function(function_type),
                kind: SymbolKind::Function,
                function_signature,
            },
        );
    }

    duplicate_implementation
}

/// Whether `noUnusedParameters` reporting applies in the current file. Skips
/// declaration files (ambient / `.d.ts`), where tsc never reports.
pub(crate) fn should_track_unused_parameters(ctx: &CheckerContext) -> bool {
    ctx.options.no_unused_parameters && ctx.current_file_kind == FileKind::RootSource
}

/// Whether the body read `name`. `reads` is the parser's sorted, deduplicated
/// read list (see `ParsedFunctionDeclaration::body_reads`), so this binary-
/// searches instead of scanning: the callers ask once per binding, and a linear
/// scan made unused-binding reporting quadratic in function size.
fn body_reads_name(reads: &[String], name: &str) -> bool {
    reads
        .binary_search_by(|read| read.as_str().cmp(name))
        .is_ok()
}

/// Checked once per reporting call rather than per binding, so verifying the
/// order the binary search depends on does not itself reintroduce the linear
/// scan in debug builds.
fn debug_assert_reads_sorted(reads: &[String]) {
    debug_assert!(
        reads.windows(2).all(|pair| pair[0] < pair[1]),
        "body_reads must stay sorted and deduplicated for the binary search"
    );
}

/// Reports TS6133 for each identifier parameter whose name never appears in the
/// body's collected reads (and is not `_`-prefixed), and walks destructuring
/// patterns for their unused bindings. The `this` pseudo-parameter is skipped.
pub(crate) fn emit_unused_parameters(
    parameters: &[ParsedFunctionParameter],
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    debug_assert_reads_sorted(reads);
    for parameter in parameters {
        match &parameter.binding_name {
            ParsedBindingName::Identifier { name, span } => {
                if name == "this" || name.starts_with('_') || body_reads_name(reads, name) {
                    continue;
                }
                let diagnostic = Diagnostic::ts6133(name, ctx.file_name.clone());
                let diagnostic = match span {
                    Some(span) => diagnostic.with_span(convert_span(*span)),
                    None => diagnostic,
                };
                ctx.push(diagnostic);
            }
            ParsedBindingName::ObjectPattern(pattern) => {
                emit_unused_object_pattern_bindings(pattern, reads, ctx);
            }
            ParsedBindingName::ArrayPattern(pattern) => {
                emit_unused_array_pattern_bindings(pattern, reads, ctx);
            }
            ParsedBindingName::Unsupported { .. } => {}
        }
    }
}

/// One unused binding found inside a destructuring pattern: the name tsc
/// renders (the *local* one, so `{ a: x }` reports `x`) and where it points.
struct UnusedBinding<'a> {
    name: &'a str,
    span: Option<TextSpan>,
}

/// Reports the unused bindings of one pattern level. tsc groups them per
/// pattern: when every element of the pattern is unused it collapses the whole
/// group into a single TS6198 on the pattern, and otherwise names each binding
/// with TS6133. A one-element group is always named, which is why
/// `({ a }: O) => 1` reports `'a'` and `({ a, b }: O) => 1` reports the
/// collapsed form.
fn report_pattern_group(
    unused: Vec<UnusedBinding<'_>>,
    element_count: usize,
    pattern_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    if unused.is_empty() {
        return;
    }
    if unused.len() >= 2 && unused.len() == element_count {
        let diagnostic = Diagnostic::ts6198(ctx.file_name.clone());
        let diagnostic = match pattern_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return;
    }
    for binding in unused {
        let diagnostic = Diagnostic::ts6133(binding.name, ctx.file_name.clone());
        let diagnostic = match binding.span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

fn emit_unused_object_pattern_bindings(
    pattern: &ParsedObjectBindingPattern,
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    let mut unused = Vec::new();
    // `const { a, ...rest } = o` uses `a` to keep it *out* of `rest`, so tsc
    // never reports the named siblings of an object rest — only the rest
    // binding itself can be unused. Array rest has no such role.
    let has_rest = pattern.rest.is_some();
    for element in &pattern.elements {
        match &element.binding_name {
            ParsedBindingName::Identifier { name, span } => {
                // `_`-prefixing exempts an object binding only when it renames
                // a property (`{ a: _a }`); shorthand `{ _a }` still reports.
                let renamed_to_ignore = !element.shorthand && name.starts_with('_');
                if has_rest || renamed_to_ignore || body_reads_name(reads, name) {
                    continue;
                }
                unused.push(UnusedBinding {
                    name,
                    span: span.or(element.name_span),
                });
            }
            ParsedBindingName::ObjectPattern(nested) => {
                emit_unused_object_pattern_bindings(nested, reads, ctx);
            }
            ParsedBindingName::ArrayPattern(nested) => {
                emit_unused_array_pattern_bindings(nested, reads, ctx);
            }
            ParsedBindingName::Unsupported { .. } => {}
        }
    }
    collect_unused_rest(pattern.rest.as_deref(), reads, false, &mut unused);
    let element_count = pattern.elements.len() + usize::from(has_rest);
    report_pattern_group(unused, element_count, pattern.span, ctx);
}

fn emit_unused_array_pattern_bindings(
    pattern: &ParsedArrayBindingPattern,
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    let mut unused = Vec::new();
    for element in pattern.elements.iter().flatten() {
        match element {
            ParsedBindingName::Identifier { name, span } => {
                if name.starts_with('_') || body_reads_name(reads, name) {
                    continue;
                }
                unused.push(UnusedBinding { name, span: *span });
            }
            ParsedBindingName::ObjectPattern(nested) => {
                emit_unused_object_pattern_bindings(nested, reads, ctx);
            }
            ParsedBindingName::ArrayPattern(nested) => {
                emit_unused_array_pattern_bindings(nested, reads, ctx);
            }
            ParsedBindingName::Unsupported { .. } => {}
        }
    }
    collect_unused_rest(pattern.rest.as_deref(), reads, true, &mut unused);
    let element_count = pattern.elements.len() + usize::from(pattern.rest.is_some());
    report_pattern_group(unused, element_count, pattern.span, ctx);
}

/// An array rest (`[..._rest]`) honours the `_` exemption like any array
/// element; an object rest (`{ ..._rest }`) renames nothing, so it does not.
fn collect_unused_rest<'a>(
    rest: Option<&'a ParsedBindingName>,
    reads: &[String],
    underscore_exempts: bool,
    unused: &mut Vec<UnusedBinding<'a>>,
) {
    let Some(ParsedBindingName::Identifier { name, span }) = rest else {
        return;
    };
    if (underscore_exempts && name.starts_with('_')) || body_reads_name(reads, name) {
        return;
    }
    unused.push(UnusedBinding { name, span: *span });
}

/// Reports TS6133 for each function-local `const`/`let`/`var`, and TS6196 for
/// each body-local `type`/`interface`, whose name never appears in the body's
/// reads. Gated on `noUnusedLocals` in a root source file. Uses the
/// function-wide read set, so a binding read in any nested scope counts (an
/// over-approximation — never a false positive).
pub(crate) fn emit_unused_locals(
    statements: &[ParsedFunctionBodyStatement],
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_unused_locals || ctx.current_file_kind != FileKind::RootSource {
        return;
    }
    debug_assert_reads_sorted(reads);
    let mut locals: Vec<(&str, Option<TextSpan>)> = Vec::new();
    let mut local_types: Vec<(&str, Option<TextSpan>)> = Vec::new();
    collect_local_var_declarations(statements, &mut locals);
    collect_local_type_declarations(statements, &mut local_types);
    for (name, span) in locals {
        if body_reads_name(reads, name) {
            continue;
        }
        let diagnostic = Diagnostic::ts6133(name, ctx.file_name.clone());
        let diagnostic = match span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
    for (name, span) in local_types {
        if body_reads_name(reads, name) {
            continue;
        }
        let diagnostic = Diagnostic::ts6196(name, ctx.file_name.clone());
        let diagnostic = match span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// Body-local `type`/`interface` declarations, recursing through control flow
/// but not into nested functions, mirroring
/// [`collect_local_var_declarations`]. A body-local `class` is a value and
/// reports TS6133 through the declaration path instead.
fn collect_local_type_declarations<'a>(
    statements: &'a [ParsedFunctionBodyStatement],
    out: &mut Vec<(&'a str, Option<TextSpan>)>,
) {
    for statement in statements {
        match statement {
            ParsedFunctionBodyStatement::TypeAlias(alias) if !alias.is_declare => {
                out.push((alias.name.as_str(), alias.name_span));
            }
            ParsedFunctionBodyStatement::Interface(interface) if !interface.is_declare => {
                out.push((interface.name.as_str(), interface.name_span));
            }
            ParsedFunctionBodyStatement::Block(body) => {
                collect_local_type_declarations(body, out);
            }
            ParsedFunctionBodyStatement::If(statement) => {
                collect_local_type_declarations(&statement.then_body, out);
                collect_local_type_declarations(&statement.else_body, out);
            }
            ParsedFunctionBodyStatement::While(statement) => {
                collect_local_type_declarations(&statement.body, out);
            }
            ParsedFunctionBodyStatement::ForOf(statement) => {
                collect_local_type_declarations(&statement.body, out);
            }
            ParsedFunctionBodyStatement::Switch(statement) => {
                for case in &statement.cases {
                    collect_local_type_declarations(&case.consequent, out);
                }
            }
            ParsedFunctionBodyStatement::Try(statement) => {
                collect_local_type_declarations(&statement.block, out);
                if let Some(handler) = &statement.handler {
                    collect_local_type_declarations(&handler.body, out);
                }
                collect_local_type_declarations(&statement.finalizer, out);
            }
            _ => {}
        }
    }
}

/// Collects `const`/`let`/`var` declarations directly owned by this function
/// body, recursing through control-flow statements but not into nested functions
/// (whose locals belong to their own scope).
fn collect_local_var_declarations<'a>(
    statements: &'a [ParsedFunctionBodyStatement],
    out: &mut Vec<(&'a str, Option<TextSpan>)>,
) {
    for statement in statements {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable)
                if !variable.is_declare
                    // tsc exempts an `_`-prefixed *destructured* binding: it is
                    // the idiom for naming a property only to drop it from a
                    // rest spread (`const { a: _a, ...rest } = x`).
                    && !(variable.from_binding_pattern && variable.name.starts_with('_')) =>
            {
                out.push((variable.name.as_str(), variable.name_span));
            }
            ParsedFunctionBodyStatement::Block(body) => collect_local_var_declarations(body, out),
            ParsedFunctionBodyStatement::If(statement) => {
                collect_local_var_declarations(&statement.then_body, out);
                collect_local_var_declarations(&statement.else_body, out);
            }
            ParsedFunctionBodyStatement::While(statement) => {
                collect_local_var_declarations(&statement.body, out)
            }
            ParsedFunctionBodyStatement::ForOf(statement) => {
                collect_local_var_declarations(&statement.body, out)
            }
            ParsedFunctionBodyStatement::Switch(statement) => {
                for case in &statement.cases {
                    collect_local_var_declarations(&case.consequent, out);
                }
            }
            ParsedFunctionBodyStatement::Try(statement) => {
                collect_local_var_declarations(&statement.block, out);
                if let Some(handler) = &statement.handler {
                    collect_local_var_declarations(&handler.body, out);
                }
                collect_local_var_declarations(&statement.finalizer, out);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_function_body_with_signature(
    name: String,
    parameters: Vec<ParsedFunctionParameter>,
    body: Vec<ParsedFunctionBodyStatement>,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    function_signature: Option<Arc<FunctionSignatureInfo>>,
    has_explicit_return_type: bool,
    missing_return_span: Option<TextSpan>,
    body_reads: Option<&[String]>,
    is_generator: bool,
    has_this_parameter: bool,
    this_parameter_type: Option<ParsedType>,
    ctx: &mut CheckerContext,
) {
    let this_type = this_parameter_type.map(|this_parameter_type| {
        with_type_parameter_scope(type_parameters, ctx, |ctx| {
            crate::infer::map_parsed_type(this_parameter_type, ctx)
        })
    });
    check_function_body_with_signature_and_this(
        Some(name),
        parameters,
        body,
        function_type,
        type_parameters,
        function_signature,
        has_explicit_return_type,
        missing_return_span,
        this_type,
        false,
        body_reads,
        is_generator,
        has_this_parameter,
        ctx,
    );
}

/// Like [`check_function_body_with_signature`], but optionally binds a `this`
/// symbol (the class instance or static side) into the body scope so class
/// method and constructor bodies can resolve `this.<member>` references.
#[allow(clippy::too_many_arguments)]
/// `name` is the body's *self* binding, so a function declaration can call
/// itself. It is `None` for a class method or constructor: the name is a member,
/// not a lexical binding, so an outer function of the same name must stay visible
/// (zod's `process` method calls the imported `process`).
pub(crate) fn check_function_body_with_signature_and_this(
    name: Option<String>,
    parameters: Vec<ParsedFunctionParameter>,
    body: Vec<ParsedFunctionBodyStatement>,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    function_signature: Option<Arc<FunctionSignatureInfo>>,
    has_explicit_return_type: bool,
    missing_return_span: Option<TextSpan>,
    this_type: Option<Type>,
    is_constructor: bool,
    body_reads: Option<&[String]>,
    is_generator: bool,
    // `function f(this: T)`: oxc keeps the `this` parameter out of the parameter
    // list, so the caller has to report whether one was written.
    has_this_parameter: bool,
    ctx: &mut CheckerContext,
) {
    let body_flow = analyze_function_body_flow(&body);
    let flow_facts = collect_function_flow_facts(&body);
    // Whether a `default`-less switch covers its discriminant is only known once
    // the body is checked, so such a body is kept to summarize again afterwards.
    let recheck_body = (body_flow.guarantees_value_return || body_flow.guarantees_exit)
        .then(|| body_has_defaultless_switch(&body).then(|| body.clone()))
        .flatten();
    let tail_call = crate::checks::expr::tail_call_key(&body);

    let root = match ctx.nested_function_scope.take() {
        Some(enclosing) => SymbolTable::with_parent(enclosing),
        None => merged_function_body_root_symbols(ctx),
    };
    let mut scopes = ScopeStack::from_root(root);
    scopes.mark_function_boundary();
    if let Some(name) = name {
        scopes.insert_current(
            name,
            SymbolInfo {
                ty: Type::Function(with_type_copy_reason(
                    TypeCopyReason::FunctionBodySetup,
                    || function_type.clone(),
                )),
                kind: SymbolKind::Function,
                function_signature,
            },
        );
    }
    // A plain `function` with no `this` parameter gives `this` no type; tsc
    // reports a read of it under `noImplicitThis`. A constructor and a method
    // both arrive here with a `this` type, which clears it.
    let outer_this_is_implicitly_any = ctx.this_is_implicitly_any;
    ctx.this_is_implicitly_any =
        this_type.is_none() && !is_constructor && !has_this_parameter;
    let outer_constructor_writable_members = if is_constructor {
        None
    } else {
        ctx.constructor_writable_members.take()
    };

    if let Some(this_type) = this_type {
        scopes.insert_current(
            "this".to_string(),
            SymbolInfo {
                ty: this_type,
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
    scopes.push_function_scope();
    super::bind_arguments_object(&mut scopes, ctx);
    let mut flow_state = FunctionFlowState::new(
        flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
    );
    let saved_never_initialized =
        crate::flow::enter_container(type_parameters, &parameters, &body, &mut flow_state, ctx);
    crate::flow::check_parameter_default_flow(&parameters, &flow_state, ctx);
    flow_state.hoist_vars(
        crate::flow::collect_hoisted_vars(&body)
            .into_iter()
            .filter(|name| !crate::flow::binds_parameter(&parameters, name))
            .collect(),
    );

    // `None` is an overload signature (no body); tsc never flags its parameters
    // or locals.
    if let Some(reads) = body_reads {
        if !is_constructor && should_track_unused_parameters(ctx) {
            emit_unused_parameters(&parameters, reads, ctx);
        }
        emit_unused_locals(&body, reads, ctx);
    }

    for (parameter, parameter_type) in parameters
        .iter()
        .zip(function_type.parameters().iter())
    {
        insert_parameter_bindings(parameter, parameter_type, &mut scopes);
    }
    // A default is written inside the signature, so the function's own type
    // parameters are in scope for it.
    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        for (parameter, parameter_type) in parameters.iter().zip(function_type.parameters().iter())
        {
            check_binding_pattern_defaults(
                &parameter.binding_name,
                Some(parameter_type),
                &scopes,
                ctx,
            );
        }
    });

    let returned_void_like = with_type_parameter_scope(type_parameters, ctx, |ctx| {
        // A declaration's own frame, never active — it has a real signature, so
        // its returns are checked. Opening one stops a nested declaration from
        // recording into an enclosing arrow's frame.
        ctx.open_contextual_return_frame();
        // Every caller is a declaration or a class member, neither of which is
        // ever contextually typed, so an unannotated one's returns relate to
        // nothing — Go checks a return only against the annotation.
        if !has_explicit_return_type {
            ctx.mark_unannotated_declaration_body();
        }
        check_function_body(
            body,
            has_explicit_return_type.then(|| function_type.return_type()),
            &mut scopes,
            &mut flow_state,
            ctx,
        );
        ctx.close_contextual_return_frame()
    });
    ctx.inherited_never_initialized = saved_never_initialized;

    if !is_constructor {
        ctx.constructor_writable_members = outer_constructor_writable_members;
    }

    // A generator's declared type describes what it *yields*, so tsc requires no
    // `return` and reports neither TS2355 nor TS7030 on it.
    if is_generator {
        return;
    }

    let body_flow = match recheck_body {
        Some(body)
            if !ctx.non_exhaustive_switches.is_empty() || !ctx.exhaustive_switches.is_empty() =>
        {
            crate::flow::with_non_exhaustive_switches(
                &ctx.non_exhaustive_switches,
                &ctx.exhaustive_switches,
                || {
                analyze_function_body_flow(&body)
            })
        }
        _ => body_flow,
    };
    let exits_via_never_call =
        tail_call.is_some_and(|key| ctx.never_returning_calls.contains(&key));
    if exits_via_never_call {
        return;
    }

    if has_explicit_return_type && should_check_missing_return(function_type.return_type()) {
        emit_missing_return_diagnostic(
            body_flow,
            function_type.return_type(),
            missing_return_span,
            ctx,
        );
    } else if !has_explicit_return_type
        && !is_constructor
        && !returned_void_like
        && ctx.options.no_implicit_returns
        && body_flow.contains_return_with_value
        && !body_flow.guarantees_exit
    {
        emit_implicit_return_diagnostic(missing_return_span, ctx);
    }

    ctx.this_is_implicitly_any = outer_this_is_implicitly_any;
}

pub(crate) fn merged_function_body_root_symbols(ctx: &CheckerContext) -> SymbolTable {
    // A function body sees its module's symbols layered over the ambient globals.
    // Rather than copying every visible symbol into a fresh table on each function
    // (O(module symbols) per function, so O(N^2) for a file with N functions), share
    // them by `Arc` as a lookup-only parent chain: locals -> module -> ambient. The
    // scope still inserts its own name/params/locals into the (empty) own map, so
    // lookups and shadowing behave exactly as with the previously flattened table.
    let ambient = Arc::new(
        ctx.ambient_global_symbols
            .clone_with_reason(TypeCopyReason::FunctionBodySetup),
    );
    let module_over_ambient = Arc::new(
        ctx.symbols
            .clone_with_reason(TypeCopyReason::FunctionBodySetup)
            .with_parent_fallback(ambient),
    );
    SymbolTable::with_parent(module_over_ambient)
}

pub(crate) fn body_has_defaultless_switch(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(|statement| match statement {
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            switch_statement.cases.iter().all(|case| case.test.is_some())
        }
        ParsedFunctionBodyStatement::Block(block) => body_has_defaultless_switch(block),
        ParsedFunctionBodyStatement::If(if_statement) => {
            body_has_defaultless_switch(&if_statement.then_body)
                || body_has_defaultless_switch(&if_statement.else_body)
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            body_has_defaultless_switch(&try_statement.block)
                || try_statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| body_has_defaultless_switch(&handler.body))
                || body_has_defaultless_switch(&try_statement.finalizer)
        }
        _ => false,
    })
}
