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
        || (!tsc_implicit_any_rule()
            && (ctx.unmodelled_jsx_props_depth > 0 || ctx.degraded_expected_type_depth > 0))
    {
        return;
    }
    if let Some(initializer) = &parameter.initializer {
        if contextual_type.is_none()
            && let ParsedBindingName::ArrayPattern(pattern) = &parameter.binding_name
        {
            emit_padded_array_binding_diagnostics(pattern, initializer, ctx);
        }
        return;
    }

    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            if contextual_type.is_some() {
                return;
            }
            // A rest parameter's implicit type is `any[]` (TS7019), reported
            // from its `...`, which the lowered parameter keeps no span for:
            // it sits right before the name.
            let (diagnostic, span) = if parameter.rest {
                let span = span.map(|span| surge_ts_syntax::TextSpan {
                    start: span.start.saturating_sub(3),
                    end: span.end,
                });
                (Diagnostic::ts7019(name, ctx.file_name.clone()), span)
            } else {
                (Diagnostic::ts7006(name, ctx.file_name.clone()), *span)
            };
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
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
    // tsc's `getTypeFromBindingElement` types an element with an initializer
    // from it; only one without is an implicit `any`.
    for (index, element) in pattern.elements.iter().enumerate() {
        if let Some(element) = element
            && !pattern.defaults.get(index).copied().unwrap_or(false)
        {
            emit_array_binding_element_diagnostic(element, ctx);
        }
    }
}

/// A parameter's array literal initializer contextually typed by its binding
/// pattern is padded to the pattern's length (tsc's `checkArrayLiteral`), and
/// a padded position without its own default is an implicit `any` that
/// `reportErrorsFromWidening` reports at the binding element.
fn emit_padded_array_binding_diagnostics(
    pattern: &ParsedArrayBindingPattern,
    initializer: &surge_ts_syntax::ParsedExpression,
    ctx: &mut CheckerContext,
) {
    let surge_ts_syntax::ParsedExpression::ArrayLiteral { elements, .. } = initializer else {
        return;
    };
    if elements.iter().any(|element| element.spread) {
        return;
    }
    for (index, element) in pattern.elements.iter().enumerate().skip(elements.len()) {
        if let Some(element @ ParsedBindingName::Identifier { .. }) = element
            && !pattern.defaults.get(index).copied().unwrap_or(false)
        {
            emit_array_binding_element_diagnostic(element, ctx);
        }
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
        // tsc's `getTypeFromBindingElement` types an element with an
        // initializer from it; only one without is an implicit `any`.
        ParsedBindingName::Identifier { .. } if element.has_default => {}
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
    // The initializer stands in for `undefined`, so the body never sees it —
    // whether the signature carries it (`(a = 1, b: T)`) or the annotation
    // wrote it (`a: T | undefined = v`).
    // An initializer that can itself be `undefined` fills nothing.
    let initializer_fills_gap = parameter.initializer.as_ref().is_some_and(|initializer| {
        !matches!(
            initializer,
            surge_ts_syntax::ParsedExpression::UndefinedLiteral
                | surge_ts_syntax::ParsedExpression::Unary {
                    operator: surge_ts_syntax::ParsedUnaryOperator::Void,
                    ..
                }
        ) && !matches!(initializer, surge_ts_syntax::ParsedExpression::Identifier { name, .. } if name == "undefined")
    });
    let without_default_gap;
    let parameter_type = if initializer_fills_gap && matches!(parameter_type, Type::Union(_)) {
        without_default_gap = surge_ts_types::remove_undefined(parameter_type);
        &without_default_gap
    } else {
        parameter_type
    };
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
            scopes.record_tuple_destructure(name, None);
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
        insert_binding_name(rest, array_rest_binding_type(&source_type), scopes);
    }
}

/// `[a, ...rest]` binds `rest` to the remaining elements. We model it as an
/// array of the source element type (or the source as-is) — precise enough to
/// keep `rest` usable without an exact `slice` shape.
fn array_rest_binding_type(source: &Type) -> Type {
    match source {
        Type::Array(_) => source.clone(),
        Type::Tuple(elements) => Type::Array(Box::new(elements.last().cloned().unwrap_or(Type::Any))),
        _ => Type::Any,
    }
}

/// The type a destructured parameter gives one of its names, read from the
/// parameter's type by the rules the body scope binds it with.
pub(crate) fn bound_name_type(
    parameter_type: &Type,
    bound: &surge_ts_syntax::ParsedBoundName,
) -> Type {
    use surge_ts_syntax::ParsedBindingStep;
    let mut ty = with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone());
    for step in &bound.path {
        ty = match step {
            ParsedBindingStep::Property(name) => object_binding_element_type(&ty, name),
            ParsedBindingStep::Index(index) => array_binding_element_type(&ty, *index),
            ParsedBindingStep::ObjectRest => ty,
            ParsedBindingStep::ArrayRest => array_rest_binding_type(&ty),
        };
    }
    if bound.has_default {
        surge_ts_types::remove_undefined(&ty)
    } else {
        ty
    }
}

/// Evaluates the defaults a destructuring pattern writes (`{ c = fallback }`)
/// in the scope the pattern binds into, for their own diagnostics. Where the
/// bound type is known this is tsc's `checkBindingElement`: the initializer of
/// `{ a = value }` is contextually typed by, and has to be assignable to, the
/// type the pattern reads at that position.
pub(crate) fn check_binding_pattern_defaults(
    binding: &ParsedBindingName,
    bound_type: Option<&Type>,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    match binding {
        ParsedBindingName::ObjectPattern(pattern) => {
            for element in &pattern.elements {
                let read_type =
                    bound_type.map(|ty| object_binding_element_type(ty, &element.property_name));
                // A nested pattern over a possibly-missing property with no
                // default of its own is the error (TS2339), and tsc checks
                // nothing beneath it.
                if let Some(read_type) = &read_type
                    && element.default_value.is_none()
                    && !matches!(element.binding_name, ParsedBindingName::Identifier { .. })
                    && !matches!(read_type, Type::Any)
                    && !read_type.is_unknown()
                    && (surge_ts_types::is_assignable_to(&Type::Undefined, read_type)
                        || surge_ts_types::is_assignable_to(&Type::Null, read_type))
                {
                    if ctx.options.strict_null_checks
                        && !crate::checks::function::type_contains_unknown(read_type)
                        && let ParsedBindingName::ObjectPattern(nested) = &element.binding_name
                    {
                        for nested_element in &nested.elements {
                            ctx.push(crate::spans::diagnostic_with_syntax_span(
                                Diagnostic::ts2339(
                                    &nested_element.property_name,
                                    read_type.name(),
                                    ctx.file_name.clone(),
                                ),
                                nested_element.span,
                            ));
                        }
                    }
                    continue;
                }
                // The destructured member's own type is the default's
                // contextual type, which is what types an arrow default's
                // parameters (`{ reducer = (items, chunk) => … }`).
                let element_type = read_type.map(|ty| surge_ts_types::remove_undefined(&ty));
                if let Some(default_value) = element.default_value.as_deref() {
                    check_binding_element_default(
                        default_value,
                        element,
                        element_type.as_ref(),
                        scopes,
                        ctx,
                    );
                }
                check_binding_pattern_defaults(
                    &element.binding_name,
                    element_type.as_ref(),
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

fn check_binding_element_default(
    default_value: &surge_ts_syntax::ParsedExpression,
    element: &ParsedObjectBindingElement,
    element_type: Option<&Type>,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    let diagnostics_before = ctx.diagnostics().len();
    let default_type = crate::checks::expected::evaluate_expression_with_expected_type(
        default_value,
        element.default_span,
        element_type,
        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
        scopes.visible_symbols(),
        ctx,
    );
    let Some(element_type) = element_type.filter(|ty| !matches!(ty, Type::Any) && !ty.is_unknown())
    else {
        return;
    };
    // A mismatch inside the initializer is reported where it is; one of the
    // whole value is reported here, on the binding element.
    if ctx.diagnostics().len() == diagnostics_before
        && let InferredExpression::Known(default_type) = default_type
        && !default_type.is_unknown()
        && !crate::checks::function::type_contains_unknown(&default_type)
        && !crate::checks::function::type_contains_unknown(element_type)
        && !surge_ts_types::is_assignable_to(&default_type, element_type)
    {
        let reported_target =
            crate::checks::expr::reported_relation_target(&default_type, element_type);
        let source_name = crate::checks::expr::source_display_name(&default_type, &reported_target);
        ctx.push(crate::spans::diagnostic_with_syntax_span(
            crate::checks::expr::type_not_assignable_diagnostic(
                &default_type,
                &reported_target,
                &source_name,
                &reported_target.name(),
                ctx.file_name.clone(),
            ),
            element.span.or(element.default_span),
        ));
    }
}

pub(crate) fn insert_parameter_bindings(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
    scopes: &mut ScopeStack,
) {
    let scope_type = parameter_scope_type(parameter, parameter_type);
    // `x: T | undefined = v` reads `T` but is still declared `T | undefined`:
    // `x = undefined` in the body is a valid write.
    if let ParsedBindingName::Identifier { name, .. } = &parameter.binding_name
        && parameter.initializer.is_some()
        && scope_type != *parameter_type
    {
        scopes.record_tuple_destructure(name, None);
        let _ = scopes.insert_current_narrowed(
            name.as_str(),
            SymbolInfo {
                ty: scope_type,
                kind: SymbolKind::Parameter,
                function_signature: None,
            },
            parameter_type.clone(),
        );
        return;
    }
    insert_binding_name(&parameter.binding_name, scope_type, scopes);
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
        // `function f({ kind, payload }: Action)`: the names are dependent when
        // the parameter is a union, exactly as a destructuring `const` is. A
        // name with a default is not a plain read of its property.
        if let (ParsedBindingName::Identifier { name, .. }, false, Some(span)) =
            (&element.binding_name, element.has_default, pattern.span)
            && matches!(parameter_type.peeled(), Type::Union(_))
        {
            scopes.record_tuple_destructure(
                name,
                Some(crate::symbols::TupleDestructureBinding {
                    source: format!("\0pattern@{}", span.start).into(),
                    key: crate::symbols::DestructureKey::Property(
                        element.property_name.as_str().into(),
                    ),
                    source_type: Some(parameter_type.clone()),
                }),
            );
        }
    }
    // `{ a, ...rest }` binds `rest` to the remaining properties.
    if let Some(rest) = &pattern.rest {
        let omitted: Vec<String> = pattern
            .elements
            .iter()
            .map(|element| element.property_name.clone())
            .collect();
        insert_binding_name(
            rest,
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                object_rest_type(&parameter_type, &omitted)
            }),
            scopes,
        );
    }
}

/// tsc's `getBindingElementTypeFromParentType` rule for `{ ...rest }`: the
/// source must be an object type (`isValidSpreadType`, with `unknown` refused
/// outright), else the binding is TS2700 and the error type. `None` for a
/// source surge could not resolve.
pub(crate) fn rest_source_validity(source: &Type) -> Option<bool> {
    fn is_definitely_falsy(ty: &Type) -> bool {
        match ty {
            Type::Null | Type::Undefined | Type::Void | Type::BooleanLiteral(false) => true,
            Type::StringLiteral(text) => text.is_empty(),
            Type::NumberLiteral(literal) => literal.value == "0",
            _ => false,
        }
    }
    match source.peeled() {
        Type::Unknown | Type::ErrorType => None,
        Type::GenuineUnknown => Some(false),
        Type::Any
        | Type::Object(_)
        | Type::Function(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::OpenTuple(_) => Some(true),
        // `getBaseConstraintOrType`: an unconstrained variable stands for itself.
        Type::TypeParameter(parameter) => match surge_ts_types::type_variable::active_constraint(&parameter) {
            Some(Some(constraint)) => rest_source_validity(&constraint),
            Some(None) => Some(true),
            None => None,
        },
        Type::Union(union) => {
            let mut valid = Some(true);
            for member in union.types().iter().filter(|member| !is_definitely_falsy(member)) {
                match rest_source_validity(member) {
                    None => return None,
                    Some(false) => valid = Some(false),
                    Some(true) => {}
                }
            }
            valid
        }
        Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::Symbol
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_) => Some(false),
        _ => None,
    }
}

/// tsc's `getRestType`: `source` without the `omitted` properties, taken
/// member by member from a union and with `undefined` dropped. A type surge
/// cannot enumerate — a sentinel, a type parameter (tsc's `Omit<T, K>`) —
/// stays as it is, and so does a non-object type.
pub(crate) fn object_rest_type(source: &Type, omitted: &[String]) -> Type {
    match source {
        Type::Reference(reference) => {
            let resolved = reference.resolve();
            if matches!(resolved, Type::Object(_) | Type::Union(_)) {
                object_rest_type(&resolved, omitted)
            } else {
                source.clone()
            }
        }
        Type::Union(union) => surge_ts_types::union_type(
            union
                .types()
                .iter()
                .filter(|member| !matches!(member, Type::Undefined))
                .map(|member| object_rest_type(member, omitted))
                .collect(),
        ),
        Type::Object(object) => {
            let properties: surge_ts_types::PropertyMap = object
                .properties
                .iter()
                .filter(|(name, _)| !omitted.iter().any(|omitted| omitted.as_str() == name.as_ref()))
                .map(|(name, property)| (name.clone(), property.clone()))
                .collect();
            let mut rest = crate::metrics::alloc_object_type(
                properties,
                object.string_index_type.as_deref().cloned(),
            )
            .with_number_index_type(object.number_index_type.as_deref().cloned());
            if object.synthetic_open_index {
                rest = rest.with_open_index_marker();
            }
            Type::Object(rest)
        }
        _ => source.clone(),
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
        // `{ 0: first }` indexes the source by the literal `0`
        // (`getLiteralTypeFromPropertyName`): a tuple's element, or an array's
        // element through its number index.
        Type::Tuple(_) | Type::Array(_) if is_array_index_name(property_name) => {
            property_name
                .parse()
                .map_or(Type::Any, |index| array_binding_element_type(source, index))
        }
        source => source
            .get_property_access_type(property_name)
            .unwrap_or(Type::Any),
    }
}

fn is_array_index_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()) && (name == "0" || !name.starts_with('0'))
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

/// The value names an expression reads: `eager` ones are evaluated while the
/// expression itself is, `deferred` ones sit inside a nested function body and
/// are read only when it runs.
#[derive(Default)]
pub(crate) struct ValueReads {
    pub(crate) eager: Vec<String>,
    pub(crate) deferred: Vec<String>,
}

pub(crate) fn collect_value_reads(
    expression: &surge_ts_syntax::ParsedExpression,
    deferred: bool,
    reads: &mut ValueReads,
) {
    use surge_ts_syntax::ParsedExpression;
    let name = match expression {
        ParsedExpression::Identifier { name, .. } => Some(name),
        ParsedExpression::Call { callee_name, .. } => Some(callee_name),
        ParsedExpression::IndexAccess { object_name, .. } => Some(object_name),
        ParsedExpression::Assignment { target_name, .. } => Some(target_name),
        ParsedExpression::JsxElement { component_name, .. } => component_name.as_ref(),
        ParsedExpression::ArrowFunction(function) => {
            collect_function_reads(function, reads);
            return;
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                if let Some(key) = &property.computed_key {
                    collect_value_reads(key, deferred, reads);
                }
                collect_value_reads(&property.value, deferred, reads);
                if let Some(setter) = &property.paired_setter {
                    collect_function_reads(setter, reads);
                }
                if let Some(value) = &property.unnamed_key_value {
                    collect_value_reads(value, deferred, reads);
                }
            }
            return;
        }
        _ => None,
    };
    if let Some(name) = name {
        if deferred {
            reads.deferred.push(name.clone());
        } else {
            reads.eager.push(name.clone());
        }
    }
    expression.for_each_child(&mut |child| collect_value_reads(child, deferred, reads));
}

fn collect_function_reads(function: &surge_ts_syntax::ParsedArrowFunction, reads: &mut ValueReads) {
    reads.deferred.extend(function.body_reads.iter().cloned());
    for parameter in &function.parameters {
        if let Some(initializer) = &parameter.initializer {
            collect_value_reads(initializer, true, reads);
        }
    }
}

/// The value names the `typeof` queries written in `ty` read. A query is eager
/// through unions, intersections, arrays, tuples, type operators, indexed
/// accesses and type arguments; inside an object type's members, a signature
/// or any other type it is resolved on demand, so deferred (the positions the
/// grammar pass's `eager_type_queries` follows for TS2502).
pub(crate) fn collect_type_query_reads(ty: &ParsedType, deferred: bool, reads: &mut ValueReads) {
    fn signature(function: &surge_ts_syntax::ParsedFunctionType, reads: &mut ValueReads) {
        for parameter in &function.parameters {
            collect_type_query_reads(&parameter.ty, true, reads);
        }
        collect_type_query_reads(&function.return_type, true, reads);
        for type_parameter in &function.type_parameters {
            for written in [&type_parameter.constraint, &type_parameter.default_type]
                .into_iter()
                .flatten()
            {
                collect_type_query_reads(written, true, reads);
            }
        }
    }
    match ty {
        ParsedType::TypeOf(query) => {
            if query.import_specifier.is_none() {
                if deferred {
                    reads.deferred.push(query.name.clone());
                } else {
                    reads.eager.push(query.name.clone());
                }
            }
            for argument in &query.type_arguments {
                collect_type_query_reads(argument, deferred, reads);
            }
        }
        ParsedType::Named(named) => {
            for argument in &named.type_arguments {
                collect_type_query_reads(argument, deferred, reads);
            }
        }
        ParsedType::Array(element) | ParsedType::Readonly(element) | ParsedType::KeyOf(element) => {
            collect_type_query_reads(element, deferred, reads);
        }
        ParsedType::Tuple(elements) | ParsedType::Union(elements) | ParsedType::Intersection(elements) => {
            for element in elements.iter() {
                collect_type_query_reads(element, deferred, reads);
            }
        }
        ParsedType::VariadicTuple(elements) => {
            for element in elements.iter() {
                match element {
                    surge_ts_syntax::ParsedTupleElement::Fixed(ty)
                    | surge_ts_syntax::ParsedTupleElement::Rest(ty, _) => {
                        collect_type_query_reads(ty, deferred, reads)
                    }
                }
            }
        }
        ParsedType::IndexedAccess(indexed_access) => {
            collect_type_query_reads(&indexed_access.object_type, deferred, reads);
            collect_type_query_reads(&indexed_access.index_type, deferred, reads);
        }
        ParsedType::Object(object) => {
            for property in &object.properties {
                collect_type_query_reads(&property.ty, true, reads);
            }
            for index in [&object.string_index_type, &object.number_index_type]
                .into_iter()
                .flatten()
            {
                collect_type_query_reads(index, true, reads);
            }
            for function in object
                .call_signature
                .iter()
                .chain(object.construct_signature.iter())
            {
                signature(function, reads);
            }
        }
        ParsedType::Function(function) => signature(function, reads),
        ParsedType::Mapped(mapped) => {
            collect_type_query_reads(&mapped.constraint, true, reads);
            collect_type_query_reads(&mapped.value_type, true, reads);
            if let Some(name_type) = mapped.name_type.as_deref() {
                collect_type_query_reads(name_type, true, reads);
            }
        }
        ParsedType::Conditional(conditional) => {
            collect_type_query_reads(&conditional.check_type, true, reads);
            collect_type_query_reads(&conditional.extends_type, true, reads);
            collect_type_query_reads(&conditional.true_type, true, reads);
            collect_type_query_reads(&conditional.false_type, true, reads);
        }
        ParsedType::TemplateLiteral(template) => {
            for interpolation in &template.interpolations {
                collect_type_query_reads(interpolation, true, reads);
            }
        }
        ParsedType::Predicate(predicate) => {
            if let Some(ty) = predicate.ty.as_ref() {
                collect_type_query_reads(ty, true, reads);
            }
        }
        ParsedType::Infer(infer) => {
            if let Some(constraint) = infer.constraint.as_ref() {
                collect_type_query_reads(constraint, true, reads);
            }
        }
        _ => {}
    }
}

/// The value names a function declaration's signature reads while it is
/// resolved: its `typeof` queries and its parameters' initializers.
pub(crate) fn signature_value_reads(
    type_parameters: &[ParsedTypeParameter],
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
) -> Vec<String> {
    let mut reads = ValueReads::default();
    for type_parameter in type_parameters {
        for written in [&type_parameter.constraint, &type_parameter.default_type]
            .into_iter()
            .flatten()
        {
            collect_type_query_reads(written, false, &mut reads);
        }
    }
    for parameter in parameters {
        if let Some(declared_type) = &parameter.declared_type {
            collect_type_query_reads(declared_type, false, &mut reads);
        }
        if let Some(initializer) = &parameter.initializer {
            collect_value_reads(initializer, false, &mut reads);
        }
    }
    if let Some(return_type) = return_type {
        collect_type_query_reads(return_type, false, &mut reads);
    }
    let ValueReads { mut eager, deferred } = reads;
    eager.extend(deferred);
    eager
}

/// Whether a signature among `functions` — the function declarations of one
/// scope, in source order — reads a function of the scope whose first
/// declaration is not before it: itself, or one declared later. A type alias or
/// interface of the scope, `local_types` with the types written in each, reads
/// what its body reads wherever a signature names it
/// (`type R = ReturnType<typeof f>; function f(): R`).
pub(crate) fn signatures_read_ahead(
    functions: &[&surge_ts_syntax::ParsedFunctionDeclaration],
    local_types: &[(&str, Vec<&ParsedType>)],
) -> bool {
    let mut first_declared: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (position, function) in functions.iter().enumerate() {
        first_declared.entry(function.name.as_str()).or_insert(position);
    }
    let local_type_reads = local_type_value_reads(local_types);
    functions.iter().enumerate().any(|(position, function)| {
        signature_reads(function, &local_type_reads)
            .iter()
            .any(|name| first_declared.get(name.as_str()).is_some_and(|first| *first >= position))
    })
}

/// The value names `function`'s signature reads, directly or through the
/// scope's local types (`local_type_reads`, from [`local_type_value_reads`]).
fn signature_reads(
    function: &surge_ts_syntax::ParsedFunctionDeclaration,
    local_type_reads: &std::collections::HashMap<&str, Vec<String>>,
) -> Vec<String> {
    let mut reads = signature_value_reads(
        &function.type_parameters,
        &function.parameters,
        function.return_type.as_ref(),
    );
    let written = function
        .parameters
        .iter()
        .filter_map(|parameter| parameter.declared_type.as_ref())
        .chain(function.return_type.as_ref())
        .chain(function.type_parameters.iter().flat_map(|parameter| {
            [&parameter.constraint, &parameter.default_type].into_iter().flatten()
        }));
    for ty in written {
        ty.for_each_named_type(&mut |named| {
            if let Some(alias_reads) = local_type_reads.get(named.name.as_str()) {
                reads.extend(alias_reads.iter().cloned());
            }
        });
    }
    reads
}

/// The order a scope's hoisting pass collects `functions` in: each name's
/// declarations after the functions their signatures read, so a signature (or
/// a local type it names) resolves against a collected signature instead of
/// the sentinel of one not collected yet — a type resolved against the sentinel
/// is memoized that way for every later reader. A name's own declarations keep
/// their source order, which is their overload order, and a cycle keeps source
/// order.
pub(crate) fn signature_collection_order(
    functions: &[&surge_ts_syntax::ParsedFunctionDeclaration],
    local_types: &[(&str, Vec<&ParsedType>)],
) -> Vec<usize> {
    let local_type_reads = local_type_value_reads(local_types);
    let mut names: Vec<&str> = Vec::new();
    for function in functions {
        if !names.contains(&function.name.as_str()) {
            names.push(function.name.as_str());
        }
    }
    let group_reads: Vec<Vec<usize>> = names
        .iter()
        .map(|name| {
            let mut reads: Vec<usize> = functions
                .iter()
                .filter(|function| function.name == *name)
                .flat_map(|function| signature_reads(function, &local_type_reads))
                .filter_map(|read| names.iter().position(|other| *other == read))
                .collect();
            reads.dedup();
            reads
        })
        .collect();
    fn visit(group: usize, group_reads: &[Vec<usize>], state: &mut [u8], order: &mut Vec<usize>) {
        if state[group] != 0 {
            return;
        }
        state[group] = 1;
        for &read in &group_reads[group] {
            visit(read, group_reads, state, order);
        }
        state[group] = 2;
        order.push(group);
    }
    let mut state = vec![0u8; names.len()];
    let mut group_order = Vec::with_capacity(names.len());
    for group in 0..names.len() {
        visit(group, &group_reads, &mut state, &mut group_order);
    }
    let mut order = Vec::with_capacity(functions.len());
    for group in group_order {
        for (index, function) in functions.iter().enumerate() {
            if function.name == names[group] {
                order.push(index);
            }
        }
    }
    order
}

/// The types an interface declaration writes in its members, index signatures
/// and heritage type arguments.
pub(crate) fn interface_written_types(
    interface: &surge_ts_syntax::ParsedInterfaceDeclaration,
) -> Vec<&ParsedType> {
    interface
        .members
        .iter()
        .map(|member| &member.ty)
        .chain(interface.string_index_type.as_ref())
        .chain(interface.number_index_type.as_ref())
        .chain(interface.extends.iter().flat_map(|heritage| heritage.type_arguments.iter()))
        .collect()
}

/// The value names each of `local_types` reads once resolved: its own `typeof`
/// queries and those of the scope's other types it names, transitively.
fn local_type_value_reads<'a>(
    local_types: &[(&'a str, Vec<&ParsedType>)],
) -> std::collections::HashMap<&'a str, Vec<String>> {
    let mut reads: std::collections::HashMap<&'a str, Vec<String>> = std::collections::HashMap::new();
    let mut names: std::collections::HashMap<&'a str, Vec<String>> = std::collections::HashMap::new();
    for (name, types) in local_types {
        let mut own = ValueReads::default();
        let entry = names.entry(name).or_default();
        for ty in types {
            collect_type_query_reads(ty, false, &mut own);
            ty.for_each_named_type(&mut |named| entry.push(named.name.clone()));
        }
        let ValueReads { eager, deferred } = own;
        reads.entry(name).or_default().extend(eager.into_iter().chain(deferred));
    }
    loop {
        let mut changed = false;
        for (name, referenced) in &names {
            let inherited: Vec<String> = referenced
                .iter()
                .filter(|other| other.as_str() != *name)
                .filter_map(|other| reads.get(other.as_str()))
                .flatten()
                .cloned()
                .collect();
            let own = reads.entry(name).or_default();
            for read in inherited {
                if !own.contains(&read) {
                    own.push(read);
                    changed = true;
                }
            }
        }
        if !changed {
            return reads;
        }
    }
}

/// The names a parameter binds, each with the type it reads from the
/// parameter's type.
fn parameter_bindings(parameter: &ParsedFunctionParameter, parameter_type: &Type) -> Vec<(String, Type)> {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, .. } => vec![(
            name.clone(),
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone()),
        )],
        binding => {
            let pattern_type = parameter_scope_type(parameter, parameter_type);
            binding
                .bound_names()
                .into_iter()
                .map(|bound| {
                    let ty = bound_name_type(&pattern_type, &bound);
                    (bound.name, ty)
                })
                .collect()
        }
    }
}

/// A parameter list as tsc's `resolveName` scopes it: the function's locals
/// hold every parameter, and `lastLocation` being a parameter makes each one
/// visible to every initializer and annotation of the list, itself and the
/// ones after it included. A parameter's type is resolved when another one
/// reads it (`getTypeOfSymbol`); a read that closes a cycle made of eager reads
/// is a circularity, and every parameter on it is `any`
/// (`reportCircularityError`). A read from inside a nested function needs the
/// type but, being deferred, never closes a cycle.
struct ParameterListResolver<'p> {
    parameters: &'p [ParsedFunctionParameter],
    bound_names: Vec<Vec<String>>,
    types: Vec<Option<Type>>,
    in_progress: Vec<bool>,
    circular: Vec<bool>,
    /// The parameters being resolved, each with whether an eager read reached it.
    stack: Vec<(usize, bool)>,
    initializer_symbols: Option<SymbolTable>,
}

impl<'p> ParameterListResolver<'p> {
    fn new(parameters: &'p [ParsedFunctionParameter]) -> Self {
        let bound_names = parameters
            .iter()
            .map(|parameter| match &parameter.binding_name {
                ParsedBindingName::Identifier { name, .. } => vec![name.clone()],
                binding => binding.bound_names().into_iter().map(|bound| bound.name).collect(),
            })
            .collect();
        Self {
            parameters,
            bound_names,
            types: vec![None; parameters.len()],
            in_progress: vec![false; parameters.len()],
            circular: vec![false; parameters.len()],
            stack: Vec::new(),
            initializer_symbols: None,
        }
    }

    fn resolve(
        &mut self,
        index: usize,
        eager: bool,
        substitution: &TypeParameterSubstitution,
        ctx: &mut CheckerContext,
    ) {
        if self.types[index].is_some() {
            return;
        }
        if self.in_progress[index] {
            if eager
                && let Some(position) = self.stack.iter().position(|(entry, _)| *entry == index)
                && self.stack[position + 1..].iter().all(|(_, eager)| *eager)
            {
                for (entry, _) in &self.stack[position..] {
                    self.circular[*entry] = true;
                }
            }
            return;
        }
        self.in_progress[index] = true;
        self.stack.push((index, eager));
        let parameters = self.parameters;
        let parameter = &parameters[index];
        let ty = if let Some(declared_type) = parameter.declared_type.clone() {
            let mut reads = ValueReads::default();
            collect_type_query_reads(&declared_type, false, &mut reads);
            self.resolve_reads(&reads.eager, true, substitution, ctx);
            self.resolve_reads(&reads.deferred, false, substitution, ctx);
            ctx.signature_parameter_bindings = self.bindings();
            let mapped = map_parsed_type_with_substitution(declared_type, ctx, substitution);
            ctx.signature_parameter_bindings.clear();
            mapped
        } else if let Some(initializer) = parameter.initializer.as_ref() {
            let mut reads = ValueReads::default();
            collect_value_reads(initializer, false, &mut reads);
            self.resolve_reads(&reads.eager, true, substitution, ctx);
            self.resolve_reads(&reads.deferred, false, substitution, ctx);
            let symbols = self.initializer_scope(ctx);
            let inferred = evaluate_expression(initializer, parameter.initializer_span, &symbols, ctx);
            self.initializer_symbols = Some(symbols);
            match inferred {
                // Only a variable initialized with `null` or `undefined` is
                // auto-typed; a parameter keeps the initializer's type, which
                // widens to `any` only without strictNullChecks.
                InferredExpression::Known(ty @ (Type::Null | Type::Undefined))
                    if ctx.options.strict_null_checks =>
                {
                    ty
                }
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
        self.stack.pop();
        self.in_progress[index] = false;
        self.types[index] = Some(if self.circular[index] { Type::Any } else { ty });
    }

    fn resolve_reads(
        &mut self,
        names: &[String],
        eager: bool,
        substitution: &TypeParameterSubstitution,
        ctx: &mut CheckerContext,
    ) {
        for index in 0..self.parameters.len() {
            if self.bound_names[index].iter().any(|bound| names.contains(bound)) {
                self.resolve(index, eager, substitution, ctx);
            }
        }
    }

    /// The scope an initializer is evaluated in: the enclosing one with every
    /// parameter bound, a parameter still being resolved as `any`.
    fn initializer_scope(&mut self, ctx: &CheckerContext) -> SymbolTable {
        let mut symbols = self.initializer_symbols.take().unwrap_or_else(|| {
            ctx.symbols
                .clone_with_reason(TypeCopyReason::FunctionBodySetup)
        });
        for (index, parameter) in self.parameters.iter().enumerate() {
            let ty = self.types[index].clone().unwrap_or(Type::Any);
            for (name, ty) in parameter_bindings(parameter, &ty) {
                let _ = symbols.insert(
                    name,
                    SymbolInfo {
                        ty,
                        kind: SymbolKind::Parameter,
                        function_signature: None,
                    },
                );
            }
        }
        symbols
    }

    /// The parameter bindings a `typeof` in the signature resolves against.
    fn bindings(&self) -> Vec<(String, Type)> {
        let mut bindings = Vec::new();
        for (index, parameter) in self.parameters.iter().enumerate() {
            let ty = match &self.types[index] {
                Some(ty) => ty.clone(),
                None if self.in_progress[index] => Type::Any,
                None => continue,
            };
            bindings.extend(parameter_bindings(parameter, &ty));
        }
        bindings
    }

    fn resolved_type(&self, index: usize) -> Type {
        self.types[index].clone().unwrap_or(Type::Any)
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

    let mut type_parameter_substitution = build_type_parameter_substitution(type_parameters);
    for type_parameter in type_parameters {
        if let Some(variable) = bound_type_variable(type_parameter, ctx) {
            type_parameter_substitution.insert_placeholder(type_parameter.name.clone(), variable);
        }
    }
    let outer_parameter_bindings = std::mem::take(&mut ctx.signature_parameter_bindings);
    let mut resolver = ParameterListResolver::new(parameters);
    for index in 0..parameters.len() {
        resolver.resolve(index, true, &type_parameter_substitution, ctx);
    }

    let mut parameter_types = Vec::with_capacity(parameters.len());
    for (index, parameter) in parameters.iter().enumerate() {
        let inferred_parameter_type = resolver.resolved_type(index);

        if let ParsedBindingName::ObjectPattern(pattern) = &parameter.binding_name
            && let Some(ParsedBindingName::Identifier { span: Some(span), .. }) =
                pattern.rest.as_deref()
            && rest_source_validity(&inferred_parameter_type) == Some(false)
        {
            ctx.push_utility_diagnostic_once(
                Diagnostic::ts2700(ctx.file_name.clone()).with_span(convert_span(*span)),
            );
        }

        // tsc's `addOptionality`: a parameter with an initializer accepts
        // `undefined` from its callers wherever it stands. A trailing one is
        // optional and widened at the call; one a required parameter follows
        // (`reducer(state = initial, action)`) carries it in its type. The
        // body binding above stays `T` — the initializer fills the gap.
        let inferred_parameter_type = if parameter.initializer.is_some()
            && ctx.options.strict_null_checks
            && index < required_parameter_count(parameters)
            && !inferred_parameter_type.is_unknown()
            && !matches!(inferred_parameter_type, Type::Any)
        {
            surge_ts_types::union_type(vec![inferred_parameter_type, Type::Undefined])
        } else {
            inferred_parameter_type
        };
        parameter_types.push(inferred_parameter_type);

        if ctx.options.no_implicit_any {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
            // `reportCircularityError`. A contextually typed parameter takes its
            // type from the context, not its initializer, so it is never circular.
            if resolver.circular[index]
                && parameter.declared_type.is_none()
                && parameter.initializer.is_some()
                && contextual_type.is_none()
                && let ParsedBindingName::Identifier {
                    name,
                    span: Some(span),
                } = &parameter.binding_name
            {
                ctx.push(
                    Diagnostic::ts7022(name, ctx.file_name.clone()).with_span(convert_span(*span)),
                );
            }
        }
    }

    ctx.signature_parameter_bindings = resolver.bindings();
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
    if let Some(ParsedType::Predicate(predicate)) = return_type {
        check_type_predicate_type(
            predicate,
            parameters,
            &parameter_types,
            &type_parameter_substitution,
            ctx,
        );
    }
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

/// tsc's `checkTypePredicate`: the predicate's type must be assignable to the
/// named parameter's type — TS2677 on the written type. A `this` predicate is
/// not related here, nor one naming the rest parameter (TS2777), nor a pair
/// with a part surge could not model.
fn check_type_predicate_type(
    predicate: &surge_ts_syntax::ParsedPredicateType,
    parameters: &[ParsedFunctionParameter],
    parameter_types: &[Type],
    substitution: &TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) {
    let (Some(written), Some(span)) = (&predicate.ty, predicate.type_span) else {
        return;
    };
    if predicate.parameter_name == "this" {
        return;
    }
    let Some(index) = parameters.iter().position(|parameter| {
        parameter_identifier_name(parameter) == Some(predicate.parameter_name.as_str())
    }) else {
        return;
    };
    let (Some(parameter), Some(parameter_type)) = (parameters.get(index), parameter_types.get(index))
    else {
        return;
    };
    if parameter.rest {
        return;
    }
    let mut parameter_type = parameter_type.clone();
    if parameter.optional && ctx.options.strict_null_checks {
        parameter_type = surge_ts_types::union_type(vec![parameter_type, Type::Undefined]);
    }
    let predicate_type = map_parsed_type_with_substitution(written.clone(), ctx, substitution);
    if !is_fully_modelled(&predicate_type) || !is_fully_modelled(&parameter_type) {
        return;
    }
    if !surge_ts_types::is_assignable_to(&predicate_type, &parameter_type) {
        ctx.push_utility_diagnostic_once(
            Diagnostic::ts2677(ctx.file_name.clone()).with_span(convert_span(span)),
        );
    }
}

/// Whether every part of `ty` a relation reads is concrete: no sentinel,
/// error type or type variable.
fn is_fully_modelled(ty: &Type) -> bool {
    fn walk(ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return true;
        }
        match ty {
            Type::Unknown | Type::ErrorType | Type::TypeParameter(_) => false,
            Type::Array(element) => walk(element, depth + 1),
            Type::Tuple(elements) => elements.iter().all(|element| walk(element, depth + 1)),
            Type::Union(union) => union.types().iter().all(|member| walk(member, depth + 1)),
            Type::Object(object) => object
                .properties
                .values()
                .all(|property| walk(&property.ty, depth + 1)),
            Type::Function(function) => {
                function
                    .parameters()
                    .iter()
                    .all(|parameter| walk(parameter, depth + 1))
                    && walk(function.return_type(), depth + 1)
            }
            Type::Reference(reference) => reference
                .arguments
                .iter()
                .all(|argument| walk(argument, depth + 1)),
            _ => true,
        }
    }
    walk(ty, 0)
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
        | ParsedType::Null
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
        | ParsedType::InferredMember(_)
        | ParsedType::UniqueSymbol(_) => false,
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
        inferred_predicate: None,
    })
}

/// [`function_signature_info`] for a declaration with a body, which may imply
/// a type predicate its signature does not write (see
/// [`FunctionSignatureInfo::inferred_predicate`]).
pub(crate) fn function_declaration_signature_info(
    function: &surge_ts_syntax::ParsedFunctionDeclaration,
    function_type: &FunctionType,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Arc<FunctionSignatureInfo> {
    let info = function_signature_info(
        &function.type_parameters,
        &function.parameters,
        function.return_type.as_ref(),
        &ctx.file_name.clone(),
    );
    if function.return_type.is_some()
        || function.is_async
        || function.is_generator
        || !function.type_parameters.is_empty()
    {
        return info;
    }
    let inferred = crate::checks::function::infer_predicate_from_function_body(
        &function.parameters,
        function_type.parameters(),
        &function.body,
        symbols,
        ctx,
    );
    with_inferred_predicate(info, inferred)
}

pub(crate) fn with_inferred_predicate(
    info: Arc<FunctionSignatureInfo>,
    inferred: Option<crate::symbols::InferredPredicate>,
) -> Arc<FunctionSignatureInfo> {
    match inferred {
        Some(inferred) => {
            let mut info = (*info).clone();
            info.inferred_predicate = Some(inferred);
            Arc::new(info)
        }
        None => info,
    }
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
        inferred_predicate: None,
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
            for written in [&parameter.constraint, &parameter.default_type]
                .into_iter()
                .flatten()
            {
                let _ = crate::infer::map_parsed_type(written.clone(), ctx);
            }
        }
    });
}

/// The variable `type_parameter` is bound to while its body is checked.
pub(crate) fn bound_type_variable(type_parameter: &ParsedTypeParameter, ctx: &CheckerContext) -> Option<Type> {
    let span = type_parameter.name_span?;
    surge_ts_types::type_variable::variable_for_declaration(&ctx.file_name, span.start as u32)
}

pub(crate) fn with_type_parameter_scope<R>(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let mut scope = std::collections::HashMap::new();
    for type_parameter in type_parameters {
        let variable = bound_type_variable(type_parameter, ctx);
        scope.insert(type_parameter.name.clone(), variable.unwrap_or(Type::Unknown));
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
        // tsc keeps one symbol for a function and the namespace (or expando
        // properties) merged with it: re-registering the declaration replaces
        // only the call signature, never the members.
        let ty = match symbols.get(&name).map(|existing| &existing.ty) {
            Some(Type::Object(object))
                if replace_existing && object.call_signature().is_some() && !object.properties.is_empty() =>
            {
                Type::Object(object.clone().with_call_signature(function_type))
            }
            _ => Type::Function(function_type),
        };
        symbols.insert(
            name,
            SymbolInfo {
                ty,
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
    let mut lists: Vec<&std::sync::Arc<surge_ts_syntax::ParsedDeclarationList>> = Vec::new();
    let mut local_types: Vec<(&str, Option<TextSpan>)> = Vec::new();
    collect_local_var_declarations(statements, &mut locals, &mut lists);
    collect_local_type_declarations(statements, &mut local_types);
    for list in lists {
        crate::program::report_unused_declaration_list(list, &|name| body_reads_name(reads, name), ctx);
    }
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
/// The function's local declarations: the lists they were written in, and
/// the declarations surge synthesized without one.
fn collect_local_var_declarations<'a>(
    statements: &'a [ParsedFunctionBodyStatement],
    out: &mut Vec<(&'a str, Option<TextSpan>)>,
    lists: &mut Vec<&'a std::sync::Arc<surge_ts_syntax::ParsedDeclarationList>>,
) {
    for statement in statements {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) if !variable.is_declare => {
                match &variable.declaration_list {
                    Some(list) => {
                        if !lists.iter().any(|seen| std::sync::Arc::ptr_eq(seen, list)) {
                            lists.push(list);
                        }
                    }
                    // tsc exempts an `_`-prefixed *destructured* binding: it is
                    // the idiom for naming a property only to drop it from a
                    // rest spread (`const { a: _a, ...rest } = x`).
                    None if variable.from_binding_pattern && variable.name.starts_with('_') => {}
                    None => out.push((variable.name.as_str(), variable.name_span)),
                }
            }
            ParsedFunctionBodyStatement::Block(body) => collect_local_var_declarations(body, out, lists),
            ParsedFunctionBodyStatement::If(statement) => {
                collect_local_var_declarations(&statement.then_body, out, lists);
                collect_local_var_declarations(&statement.else_body, out, lists);
            }
            ParsedFunctionBodyStatement::While(statement) => {
                collect_local_var_declarations(&statement.body, out, lists)
            }
            ParsedFunctionBodyStatement::ForOf(statement) => {
                collect_local_var_declarations(&statement.body, out, lists)
            }
            ParsedFunctionBodyStatement::Switch(statement) => {
                for case in &statement.cases {
                    collect_local_var_declarations(&case.consequent, out, lists);
                }
            }
            ParsedFunctionBodyStatement::Try(statement) => {
                collect_local_var_declarations(&statement.block, out, lists);
                if let Some(handler) = &statement.handler {
                    collect_local_var_declarations(&handler.body, out, lists);
                }
                collect_local_var_declarations(&statement.finalizer, out, lists);
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
    is_async: bool,
    has_this_parameter: bool,
    this_parameter_type: Option<ParsedType>,
    ctx: &mut CheckerContext,
) {
    let _this_class = crate::checks::expr::ThisParameterClassScope::enter(
        crate::checks::expr::this_parameter_class(this_parameter_type.as_ref(), type_parameters, ctx),
    );
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
        is_async,
        has_this_parameter,
        ctx,
    );
}

/// Binds a generic body's own type parameters as type variables for as long as
/// the returned scope lives, their constraints resolved inside it (a
/// constraint may name them: `U extends T`).
pub(crate) fn enter_body_type_variables(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) -> Option<surge_ts_types::type_variable::TypeVariableScope> {
    if type_parameters.is_empty() || type_parameters.iter().any(|parameter| parameter.name_span.is_none()) {
        return None;
    }
    let file: Arc<str> = Arc::from(ctx.file_name.as_str());
    let already_bound = type_parameters.iter().all(|parameter| {
        parameter.name_span.is_some_and(|span| {
            surge_ts_types::type_variable::variable_for_declaration(&file, span.start as u32).is_some()
        })
    });
    if already_bound {
        return None;
    }
    let scope = surge_ts_types::type_variable::TypeVariableScope::enter(type_parameters.iter().map(|parameter| {
        let offset = parameter.name_span.map_or(0, |span| span.start as u32);
        (Arc::from(parameter.name.as_str()), (file.clone(), offset))
    }));
    let diagnostics_before = ctx.diagnostics().len();
    let mut unmodelled = Vec::new();
    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        for parameter in type_parameters {
            if let Some(constraint) = parameter.constraint.clone() {
                let constraint = crate::infer::map_parsed_type(constraint, ctx);
                if constraint.is_unmodelled()
                    || matches!(constraint, Type::ErrorType)
                    || crate::checks::assign::type_contains_unknown(&constraint)
                {
                    unmodelled.push(parameter.name.as_str());
                } else {
                    scope.set_constraint(&parameter.name, constraint);
                }
            }
        }
    });
    for name in unmodelled {
        scope.forget(name);
    }
    // The declaration check already reported whatever its constraints raise.
    ctx.truncate_diagnostics(diagnostics_before);
    Some(scope)
}

/// The signature a generic body is checked against: re-mapped while its type
/// variables are bound, so every annotation — `Readonly<T>` included —
/// resolves over the variables rather than the placeholders the declaration
/// was collected with. The written return type is kept when there is none to
/// re-map; diagnostics were already reported by the first mapping.
fn signature_over_type_variables(
    parameters: &[ParsedFunctionParameter],
    return_annotation: Option<&ParsedType>,
    collected: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) -> FunctionType {
    let diagnostics_before = ctx.diagnostics().len();
    let remapped = map_function_signature(parameters, return_annotation, type_parameters, None, ctx);
    ctx.truncate_diagnostics(diagnostics_before);
    if remapped.parameters().len() != collected.parameters().len() {
        return collected.clone();
    }
    let return_type = if return_annotation.is_some() {
        remapped.return_type().clone()
    } else {
        collected.return_type().clone()
    };
    collected.with_signature_types(remapped.parameters().to_vec(), return_type)
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
    is_async: bool,
    // `function f(this: T)`: oxc keeps the `this` parameter out of the parameter
    // list, so the caller has to report whether one was written.
    has_this_parameter: bool,
    ctx: &mut CheckerContext,
) {
    let type_variables = enter_body_type_variables(type_parameters, ctx);
    let body_function_type;
    let function_type = match (&type_variables, function_signature.as_deref()) {
        (Some(_), Some(signature)) => {
            body_function_type = signature_over_type_variables(
                &parameters,
                signature.return_type.as_ref(),
                function_type,
                type_parameters,
                ctx,
            );
            &body_function_type
        }
        _ => function_type,
    };
    let body_flow = analyze_function_body_flow(&body);
    let flow_facts = collect_function_flow_facts(&body);
    // Whether a `default`-less switch covers its discriminant is only known once
    // the body is checked, so such a body is kept to summarize again afterwards.
    let recheck_body = (body_flow.guarantees_value_return || body_flow.guarantees_exit)
        .then(|| body_has_defaultless_switch(&body).then(|| body.clone()))
        .flatten();
    let tail_call = crate::checks::expr::tail_call_key(&body);
    // tsc's `checkReturnStatement`: without `strictNullChecks`, a bare `return;`
    // is TS7030 under `noImplicitReturns` unless the (unwrapped) return type is
    // `undefined`, `void` or `any`.
    let bare_returns = (has_explicit_return_type
        && !is_constructor
        && !is_generator
        && !ctx.options.strict_null_checks
        && ctx.options.no_implicit_returns)
        .then(|| super::body::bare_return_spans(&body))
        .unwrap_or_default();

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
    ctx.this_is_implicitly_any = this_type.is_none() && !is_constructor && !has_this_parameter;
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

    for (parameter, parameter_type) in parameters.iter().zip(function_type.parameters().iter()) {
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

    // The flow summary cannot see a callee's return type; the declared ones in
    // scope decide which call statements end the flow, as a `throw` does.
    let never_calls = super::body_statements::never_call_statements(&body, &scopes);
    let body_flow = if never_calls.is_empty() {
        body_flow
    } else {
        crate::flow::with_never_calls(&never_calls, || analyze_function_body_flow(&body))
    };

    let returned_void_like = with_type_parameter_scope(type_parameters, ctx, |ctx| {
        // A declaration's own frame, never active — it has a real signature, so
        // its returns are checked. Opening one stops a nested declaration from
        // recording into an enclosing arrow's frame.
        ctx.open_contextual_return_frame();
        let outer_async_body = std::mem::replace(&mut ctx.in_async_body, is_async);
        let outer_generator_body = std::mem::replace(&mut ctx.in_generator_body, is_generator);
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
        ctx.in_async_body = outer_async_body;
        ctx.in_generator_body = outer_generator_body;
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
                || analyze_function_body_flow(&body),
            )
        }
        _ => body_flow,
    };
    let exits_via_never_call =
        tail_call.is_some_and(|key| ctx.never_returning_calls.contains(&key));
    if exits_via_never_call {
        return;
    }

    // tsc's `unwrapReturnType`: an async body owes the awaited return type,
    // so `async (): Promise<void>` with no `return` is exempt like `(): void`.
    let unwrapped_return_type = if is_async && !is_generator {
        crate::checks::call::awaited_type(function_type.return_type())
    } else {
        function_type.return_type().clone()
    };
    if !super::body::is_undefined_void_or_any(&unwrapped_return_type) {
        for span in bare_returns {
            emit_implicit_return_diagnostic(Some(span), ctx);
        }
    }
    if has_explicit_return_type && should_check_missing_return(&unwrapped_return_type) {
        emit_missing_return_diagnostic(
            body_flow,
            &unwrapped_return_type,
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
