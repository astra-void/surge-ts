//! Property-call and optional-property-call checking.

use super::*;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCallArgument, ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{FunctionType, Type, union_type};

use crate::checks::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

/// Whether a member reads as the permissive shape a namespace value object gives
/// every member (`any`, or a function returning `any`). Used as the gate before
/// the qualified `ns.member` lookup, which must not run on every property call.
pub(crate) fn is_permissive_member_type(property_type: &Type) -> bool {
    match property_type {
        Type::Any => true,
        Type::Function(function_type) => *function_type.return_type() == Type::Any,
        _ => false,
    }
}

/// Routes `ns.member(...)` through the qualified `ns.member` binding when one
/// exists. `None` means no such binding — the caller keeps its own answer.
#[allow(clippy::too_many_arguments)]
fn try_qualified_namespace_call(
    object_name: &str,
    property_name: &str,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Option<Type>> {
    let qualified_name = format!("{object_name}.{property_name}");
    crate::infer::expression::qualified_namespace_member(&qualified_name, symbols, ctx)?;
    Some(crate::checks::call::check_call_like(
        &qualified_name,
        callee_span,
        call_span,
        type_arguments,
        arguments,
        symbols,
        ctx,
    ))
}

/// tsc still checks a call's argument expressions when the callee is `any`, so
/// their own errors surface (an untyped callback parameter is TS7006/TS7031, a
/// bad reference inside the callback body is still reported). The symmetric
/// call-position arm lives in `check_call_like_with_expected_type`.
///
/// Implicit-any is reported only when the receiver's `any` is the source's, not
/// surge's — see [`receiver_any_is_genuine`]. Everything else in the argument
/// (an unresolved name, a missing member, a mismatched body) is reported either
/// way, since those do not depend on the callback's parameter types.
pub(super) fn evaluate_arguments_context_free(
    object: &ParsedExpression,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let genuine = receiver_any_is_genuine(object, symbols, ctx);
    let saved_depth = ctx.degraded_expected_type_depth;
    if genuine {
        // tsc has no contextual parameter type here either, so it *does* report
        // these parameters. An enclosing suppression — a builder chain surge
        // could not model further out — must not hide that: the provenance of
        // this receiver is decided, and it outranks the ambient guess.
        ctx.degraded_expected_type_depth = 0;
    } else {
        ctx.degraded_expected_type_depth += 1;
    }
    for argument in arguments {
        let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
    }
    ctx.degraded_expected_type_depth = saved_depth;
}

/// Whether an `any`-typed receiver is `any` because the *source* says so, rather
/// than because surge gave up partway along the chain.
///
/// Genuine: a binding stubbed for an import whose module was reported
/// unresolved, which tsc types as its error type. A callback passed to a call on
/// it really has no contextual type, so TS7006/TS7031 on its parameters is what
/// tsc reports.
///
/// Not genuine: a chain that started from a typed binding and collapsed to `any`
/// midway (a generic builder surge could not model). tsc still contextually
/// types those callbacks, so reporting implicit-any there describes surge's gap
/// rather than the source.
pub(crate) fn receiver_any_is_genuine(
    object: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> bool {
    match object {
        ParsedExpression::Identifier { name, .. } => name_is_genuine_any(name, symbols, ctx),
        // A bare call and an index off a bare name both name their base
        // directly rather than nesting an `Identifier` node.
        ParsedExpression::Call { callee_name, .. } => {
            name_is_genuine_any(callee_name, symbols, ctx)
        }
        ParsedExpression::IndexAccess { object_name, .. } => {
            name_is_genuine_any(object_name, symbols, ctx)
        }
        // Walk the chain to the binding it started from: `p.input(x).query(cb)`
        // is genuine exactly when `p` is.
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. }
        | ParsedExpression::PropertyCall { object, .. }
        | ParsedExpression::OptionalPropertyCall { object, .. }
        | ParsedExpression::ElementAccess { object, .. } => {
            receiver_any_is_genuine(object, symbols, ctx)
        }
        // Forms that hand the operand's type straight back: `await any`,
        // `any!`, and an index off one are all still `any` to tsc. A type
        // assertion is deliberately absent — `x as Foo` states a real type.
        ParsedExpression::Await { operand, .. } => {
            receiver_any_is_genuine(operand, symbols, ctx)
        }
        ParsedExpression::NonNullAssertion { expression, .. } => {
            receiver_any_is_genuine(expression, symbols, ctx)
        }
        // A union with `any` in it is `any`, so either arm settles it:
        // `caller.list() ?? []` and `cond ? caller.x : []` both stay `any`.
        ParsedExpression::Logical { left, right, .. }
        | ParsedExpression::Binary { left, right, .. }
        | ParsedExpression::NullishCoalescing { left, right, .. } => {
            receiver_any_is_genuine(left, symbols, ctx)
                || receiver_any_is_genuine(right, symbols, ctx)
        }
        ParsedExpression::Conditional {
            when_true,
            when_false,
            ..
        } => {
            receiver_any_is_genuine(when_true, symbols, ctx)
                || receiver_any_is_genuine(when_false, symbols, ctx)
        }
        _ => false,
    }
}

fn name_is_genuine_any(name: &str, symbols: &SymbolTable, ctx: &CheckerContext) -> bool {
    symbols
        .get(name)
        .is_some_and(|symbol| matches!(symbol.kind, crate::symbols::SymbolKind::ErrorImport))
        || ctx.genuine_any_bindings.contains(name)
}

/// `Array.prototype.filter` narrows its element type when the callback is a type
/// predicate (`rows.filter(isCode)` is `Code[]`). The predicate lives on the
/// argument's collected signature, not on its resolved callable type, so it is
/// read from there; anything else falls through to the ordinary `filter` model.
fn filtered_element_type(
    arguments: &[ParsedCallArgument],
    element: &Type,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let [argument] = arguments else {
        return None;
    };
    match &argument.expression {
        ParsedExpression::Identifier { name, .. } => {
            crate::checks::function::predicate_target_of_value(name, symbols, ctx)
        }
        // An inline `(x): x is T => …` carries its predicate on the arrow's
        // written return type.
        ParsedExpression::ArrowFunction(arrow) => {
            let Some(ParsedType::Predicate(predicate)) = &arrow.return_type else {
                return inferred_predicate_target(arrow, element, symbols);
            };
            if predicate.asserts || !arrow.type_parameters.is_empty() {
                return None;
            }
            // Resolving the target here is a query the *call* makes; the arrow
            // checks its own annotation at its own span. Reporting from here is
            // therefore a second diagnostic, and it is made without the arrow's
            // parameters in scope — a predicate written in terms of the tested
            // parameter (`(c): c is Exclude<typeof c, undefined>`) has no `c` to
            // find and reported a false TS2304 on it.
            let diagnostics_before = ctx.diagnostics().len();
            let target = crate::infer::map_parsed_type(predicate.ty.clone()?, ctx);
            ctx.truncate_diagnostics(diagnostics_before);
            (!target.is_unknown()).then_some(target)
        }
        _ => None,
    }
}

/// tsc's `getTypePredicateFromBody`: a callback with no return annotation that
/// does nothing but narrow its own parameter *is* a type predicate, so
/// `xs.filter(x => x !== null)` yields `number[]` (TS 5.5). Only the first
/// parameter is read, which is the one `filter`'s predicate overload tests.
fn inferred_predicate_target(
    arrow: &surge_ts_syntax::ParsedArrowFunction,
    element: &Type,
    symbols: &SymbolTable,
) -> Option<Type> {
    if arrow.return_type.is_some()
        || arrow.is_async
        || arrow.is_generator
        || !arrow.type_parameters.is_empty()
    {
        return None;
    }
    let parameter = arrow.parameters.first()?;
    if parameter.rest {
        return None;
    }
    let surge_ts_syntax::ParsedBindingName::Identifier { name, .. } = &parameter.binding_name
    else {
        return None;
    };
    let returned = single_returned_expression(&arrow.body)?;

    let mut scope = symbols.clone();
    scope.insert(
        name.clone(),
        crate::symbols::SymbolInfo {
            ty: element.clone(),
            kind: crate::symbols::SymbolKind::Parameter,
            function_signature: None,
        },
    );
    let narrowed_type = |branch_is_true: bool| {
        crate::checks::function::narrow_condition_symbol_table(returned, &scope, branch_is_true)
            .and_then(|narrowed| narrowed.get(name).map(|symbol| symbol.ty.clone()))
    };
    let target = narrowed_type(true)?;
    let rejected = narrowed_type(false)?;
    // The false branch has to be exactly what the true branch leaves out:
    // `x => !!x` narrows `number | null` to `number` when true but proves
    // nothing when false (`0` is falsy), and tsc infers no predicate from it.
    if target.is_unknown() || !crate::checks::function::predicate_partitions(element, &target, &rejected) {
        return None;
    }
    Some(target)
}

/// The one expression a predicate body can consist of: an expression body, or a
/// block whose only statement is a `return`. tsc refuses to infer from anything
/// with more than one return.
fn single_returned_expression(
    body: &surge_ts_syntax::ParsedArrowFunctionBody,
) -> Option<&ParsedExpression> {
    match body {
        surge_ts_syntax::ParsedArrowFunctionBody::Expression(expression) => Some(expression),
        surge_ts_syntax::ParsedArrowFunctionBody::Block(statements) => match statements.as_slice() {
            [surge_ts_syntax::ParsedFunctionBodyStatement::Return(statement)] => {
                statement.expression.as_ref()
            }
            _ => None,
        },
    }
}

pub(crate) fn check_property_call_like(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let object_ty =
        match crate::checks::expr::evaluate_expression(object, object_span, symbols, ctx) {
            // `Promise<T>` is modelled as its awaited `T` (see the `.then`
            // arm below), so a `void`-resolving promise looks `undefined` to
            // the receiver check; the promise itself is never nullish.
            crate::infer::InferredExpression::Known(ty)
                if matches!(property_name, "then" | "catch" | "finally") =>
            {
                ty
            }
            crate::infer::InferredExpression::Known(ty) => {
                crate::checks::expr::strip_reported_undefined_receiver(
                    object,
                    ty,
                    object_span,
                    call_span,
                    symbols,
                    ctx,
                )
            }
            // The receiver did not resolve, but the arguments are still code:
            // an unresolved name, a missing member or an implicit-any parameter
            // inside them is reported by tsc regardless of what the callee is.
            // Dropping the call here left whole callback bodies — JSX subtrees
            // included — unchecked. `evaluate_arguments_context_free` still
            // decides *implicit-any* by receiver provenance, so a chain that
            // merely collapsed to `any` does not start reporting parameters tsc
            // contextually types.
            _ => {
                evaluate_arguments_context_free(object, arguments, symbols, ctx);
                return None;
            }
        };

    crate::checks::expr::check_member_accessibility(
        object,
        &object_ty,
        property_name,
        property_span,
        false,
        symbols,
        ctx,
    );

    let object_type_name = object_ty.name();

    // Computed before the dispatch below so the narrowed element can be matched
    // on: `filter` with a type-predicate callback yields that predicate's type.
    let filtered_element = match (&object_ty, property_name) {
        (Type::Array(element), "filter") => {
            let element = element.as_ref().clone();
            filtered_element_type(arguments, &element, symbols, ctx)
        }
        _ => None,
    };

    if property_name == "all" && is_promise_all_receiver(&object_ty) {
        return check_promise_all_call(arguments, call_span.or(property_span), symbols, ctx);
    }
    // `Promise<T>` is modeled as its awaited `T`, so a `.then`/`.catch`/`.finally`
    // chained on a promise-returning call lands on the value type and would be
    // reported as a missing member. Treat the receiver as the awaited value
    // instead — the chain keeps its collapsed result so further links resolve.
    if matches!(property_name, "then" | "catch" | "finally")
        && !matches!(object_ty, Type::Unknown | Type::TypeParameter(_))
        && !declares_own_property(&object_ty, property_name)
    {
        return if property_name == "then" {
            check_promise_then_call(object_ty, arguments, symbols, ctx)
        } else {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            Some(object_ty)
        };
    }

    // A namespace binding that has not resolved in this round reads as `any` or
    // as the sentinel; the qualified `ns.member` entry still carries the member's
    // real signature, so try it before answering from the degraded receiver.
    if matches!(object_ty, Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_))
        && let ParsedExpression::Identifier { name, .. } = object
        && let Some(result) = try_qualified_namespace_call(
            name,
            property_name,
            property_span.or(object_span),
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
    {
        return result;
    }

    match object_ty {
        Type::Any => {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            Some(Type::Any)
        }
        // The receiver degraded, but the arguments are still code — same
        // reasoning as the unresolved-receiver arm above. `evaluate_arguments_
        // context_free` keeps implicit-any gated on receiver provenance, so a
        // callback whose contextual type surge lost is still not reported.
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            None
        }
        // The predicate decides the element type; the call is still checked
        // like any other `filter`, which is what gives an inline arrow its
        // contextual parameter types.
        Type::Array(_) if filtered_element.is_some() => {
            match object_ty.get_property_access_type("filter") {
                Some(Type::Function(filter)) => {
                    check_function_type_call(
                        &filter,
                        property_span,
                        call_span,
                        type_arguments,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                }
                _ => {
                    for argument in arguments {
                        let _ =
                            evaluate_expression(&argument.expression, argument.span, symbols, ctx);
                    }
                }
            }
            filtered_element.map(|element| Type::Array(Box::new(element)))
        }
        Type::Array(element_type) if property_name == "map" => check_array_map_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        ),
        Type::Array(element_type)
            if matches!(property_name, "reduce" | "reduceRight")
                && matches!(arguments.len(), 1 | 2)
                && type_arguments.len() <= 1
                && !arguments.iter().any(|argument| argument.spread) =>
        {
            Some(check_array_reduce_call(
                element_type.as_ref(),
                type_arguments,
                arguments,
                symbols,
                ctx,
            ))
        }
        Type::Array(element_type) if property_name == "find" => check_array_find_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        ),
        Type::Union(union_type) => {
            // A sentinel member means part of the receiver is unmodelled, so a
            // miss on any *other* member says nothing about the source — the
            // same no-cascade rule a wholly-sentinel receiver gets.
            if union_type.types().iter().any(Type::is_unknown) {
                return None;
            }
            // The property is looked up on the union before anything is called:
            // a member lacking it is the error, whatever the others hold.
            let lacks_member = |ty: &Type| {
                !matches!(ty, Type::Undefined | Type::Null)
                    && ty.get_property_access_type(property_name).is_none()
                    && !ty.peeled().is_unknown()
                    && !matches!(ty, Type::Array(_) | Type::Tuple(_))
            };
            if union_type.types().iter().any(lacks_member)
                && !union_type
                    .types()
                    .iter()
                    .any(|ty| crate::checks::expr::carries_leaked_type_parameter(ty, ctx))
            {
                ctx.push(diagnostic_with_syntax_span(
                    crate::checks::expr::missing_property_diagnostic(
                        property_name,
                        &Type::Union(union_type.clone()),
                        symbols,
                        ctx.file_name.clone(),
                    ),
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            }
            let mut result_types = vec![];
            for ty in union_type.types() {
                if matches!(ty, Type::Undefined | Type::Null) {
                    result_types.push(Type::Undefined);
                    continue;
                }

                if property_name == "map"
                    && let Type::Array(element_type) = ty
                {
                    let mapped = check_array_map_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(mapped);
                    continue;
                }

                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    let found = check_array_find_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(found);
                    continue;
                }

                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    let found = check_array_find_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(found);
                    continue;
                }

                let Some(property_type) = ty.get_property_access_type(property_name) else {
                    // A member whose reference peels to the sentinel is a shape
                    // surge could not reconstruct, not a type without the member.
                    if ty.peeled().is_unknown()
                        || crate::checks::expr::carries_leaked_type_parameter(ty, ctx)
                    {
                        return None;
                    }
                    if no_lib_array_member(ty, ctx) {
                        result_types.push(Type::Any);
                        continue;
                    }
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone()),
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                let declared_member = property_type.clone();
                match callable_property_signature(property_type) {
                    Type::Function(function_type) => {
                        let function_type = instantiate_declared_member_signature(
                            &function_type,
                            Some(&declared_member),
                            type_arguments,
                            property_span,
                            arguments,
                            expected_return_type,
                            symbols,
                            ctx,
                        );
                        let return_type = check_function_type_call(
                            &function_type,
                            property_span,
                            call_span,
                            type_arguments,
                            arguments,
                            symbols,
                            ctx,
                        )?;
                        result_types.push(return_type);
                    }
                    Type::Any => result_types.push(Type::Any),
                    Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => return None,
                    // A member whose own type is a union of callables sharing
                    // one signature is callable, and one carrying the
                    // degradation sentinel proves nothing — the same rules the
                    // bare-call path applies.
                    Type::Union(union) => {
                        let return_type = super::check_callable_union_call(
                            &union,
                            property_span,
                            crate::checks::expr::element_access_span(object_span, property_span)
                                .map(|span| SyntaxTextSpan { end: span.end - 1, ..span }),
                            call_span,
                            type_arguments,
                            arguments,
                            symbols,
                            ctx,
                        )?;
                        result_types.push(return_type);
                    }
                    _ => {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts2349(ctx.file_name.clone()),
                            crate::spans::choose_span(
                                property_span,
                                crate::spans::choose_span(call_span, object_span),
                            ),
                        ));
                        return None;
                    }
                }
            }

            Some(surge_ts_types::union_type(result_types))
        }
        _ => {
            if property_name == "map"
                && let Type::Array(element_type) = &object_ty
            {
                return check_array_map_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                );
            }

            let property_type =
                (!crate::infer::expression::lib_lacks_builtin_member(&object_ty, property_name, ctx))
                    .then(|| object_ty.get_property_access_type(property_name))
                    .flatten()
                    .or_else(|| {
                        crate::infer::expression::lib_builtin_member_type(
                            &object_ty,
                            property_name,
                            ctx,
                        )
                    });
            let Some(property_type) = property_type else {
                if no_lib_array_member(&object_ty, ctx) {
                    return Some(Type::Any);
                }
                // See the matching guard in the property-access path: a nominal
                // reference peeling to the sentinel is an unreconstructed shape,
                // not a type without the member.
                if object_ty.peeled().is_unknown()
                    || crate::checks::expr::carries_leaked_type_parameter(&object_ty, ctx)
                {
                    return None;
                }
                let lib_feature =
                    crate::checks::expr::lib_feature_of_missing_member(&object_ty, property_name);
                let diagnostic = match crate::checks::expr::property_spelling_suggestion(
                    property_name,
                    &object_ty,
                ) {
                    _ if lib_feature.is_some() => Diagnostic::ts2550(
                        property_name,
                        &object_type_name,
                        lib_feature.unwrap_or_default(),
                        ctx.file_name.clone(),
                    ),
                    Some(suggestion) => Diagnostic::ts2551(
                        property_name,
                        &object_type_name,
                        suggestion,
                        ctx.file_name.clone(),
                    ),
                    None => {
                        Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone())
                    }
                };
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            // A generic namespace member is published under a qualified
            // `ns.member` key carrying its real signature; the namespace object
            // models only the member *set*, so the member itself reads as the
            // permissive `any`. Routing the call through the qualified binding
            // restores arity, return type, and generic inference. Gated on that
            // permissive shape — already in hand — so no ordinary property call
            // pays for the lookup.
            if let ParsedExpression::Identifier { name, .. } = object
                && is_permissive_member_type(&property_type)
                && let Some(result) = try_qualified_namespace_call(
                    name,
                    property_name,
                    property_span.or(object_span),
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                )
            {
                return result;
            }

            let declared_member = property_type.clone();
            match callable_property_signature(property_type) {
                Type::Function(function_type) => {
                    let function_type = instantiate_declared_member_signature(
                        &function_type,
                        Some(&declared_member),
                        type_arguments,
                        property_span,
                        arguments,
                        expected_return_type,
                        symbols,
                        ctx,
                    );
                    check_function_type_call(
                        &function_type,
                        property_span,
                        call_span,
                        type_arguments,
                        arguments,
                        symbols,
                        ctx,
                    )
                }
                Type::Any => {
                    evaluate_arguments_context_free(object, arguments, symbols, ctx);
                    Some(Type::Any)
                }
                Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => {
                    evaluate_arguments_context_free(object, arguments, symbols, ctx);
                    None
                },
                // See the union arm in the multi-receiver loop above: a property
                // typed as a union of callables is callable, and one carrying the
                // degradation sentinel is not a source error.
                Type::Union(union) => super::check_callable_union_call(
                    &union,
                    property_span,
                    crate::checks::expr::element_access_span(object_span, property_span)
                        .map(|span| SyntaxTextSpan { end: span.end - 1, ..span }),
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                ),
                _ => {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2349(ctx.file_name.clone()),
                        crate::spans::choose_span(
                            property_span,
                            crate::spans::choose_span(call_span, object_span),
                        ),
                    ));
                    None
                }
            }
        }
    }
}

fn check_promise_then_call(
    value_type: Type,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(callback) = arguments.first() else {
        return Some(Type::Unknown);
    };

    let callback_type =
        Type::Function(FunctionType::new(vec![value_type], Type::Unknown, false, 1));
    let inferred_callback = evaluate_expression_with_expected_type(
        &callback.expression,
        callback.span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    let next_value = match inferred_callback {
        InferredExpression::Known(Type::Function(function_type)) => {
            promise_like_awaited_type(function_type.return_type())
        }
        InferredExpression::Known(ty) => promise_like_awaited_type(&ty),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => Type::Unknown,
    };

    Some(next_value)
}

/// Whether `ty` answers `name` from a member of its own, as opposed to from an
/// index signature. A `.d.ts` class whose base could not be resolved is left open
/// with a permissive string index, which would otherwise make every value look
/// like a thenable.
pub(crate) fn declares_own_property(ty: &Type, name: &str) -> bool {
    if let Type::Object(object_type) = ty.peeled() {
        return object_type.get_property_type(name).is_some();
    }
    ty.get_property_access_type(name).is_some()
}

pub(crate) fn promise_like_awaited_type(ty: &Type) -> Type {
    if let Type::Reference(reference) = ty {
        let base = reference
            .display
            .split('<')
            .next()
            .unwrap_or(&reference.display);
        if matches!(base, "Promise" | "PromiseLike")
            && let Some(value_type) = reference.arguments.first()
        {
            return value_type.clone();
        }
    }

    ty.clone()
}

/// `Promise<value>` — what an async function returns for a body that
/// completes with `value` (tsc's `createPromiseReturnType`). The awaited value
/// is what gets wrapped, so a body returning a promise does not nest. Without a
/// lib `Promise` the value is kept as it is.
pub(crate) fn promise_of(value: &Type, ctx: &mut CheckerContext) -> Type {
    if !promise_nominal_enabled() {
        return value.clone();
    }
    let value = awaited_type(value);
    if value.is_unknown() {
        return value;
    }
    const SLOT: &str = "__surge_promised";
    let mut substitution = crate::infer::TypeParameterSubstitution::new();
    substitution.insert(SLOT.to_string(), value.clone());
    let named = |name: &str, type_arguments| {
        ParsedType::Named(std::sync::Arc::new(surge_ts_syntax::ParsedNamedType {
            name: name.to_string(),
            span: None,
            type_arguments,
        }))
    };
    let reported = ctx.diagnostics().len();
    let promise = crate::infer::map_parsed_type_with_substitution(
        named("Promise", vec![named(SLOT, Vec::new())]),
        ctx,
        &substitution,
    );
    ctx.truncate_diagnostics(reported);
    if promise.is_unknown() { value } else { promise }
}

/// `SURGE_PROMISE_NOMINAL=1`: keep the lib's `Promise<T>` / `PromiseLike<T>` as
/// the interface it is instead of collapsing it to its awaited `T`. The
/// collapse dates from when `await` was erased at parse time; it makes
/// `Promise<T> | undefined` read as `T | undefined` and hides every misuse of
/// a promise as its value. Off until the corpora are clean under it — see
/// CURRENT_STATUS.md, semantic compatibility gates.
pub(crate) fn promise_nominal_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_PROMISE_NOMINAL").is_some())
}

/// The type `await ty` produces, following tsc's `getAwaitedTypeNoAlias`:
/// `any` awaits to itself, a union awaits constituent-wise, and a promise is
/// unwrapped repeatedly (`Promise<Promise<T>>` awaits to `T`). Anything that is
/// not promise-like awaits to itself.
pub(crate) fn awaited_type(ty: &Type) -> Type {
    awaited_type_at_depth(ty, 0)
}

fn awaited_type_at_depth(ty: &Type, depth: usize) -> Type {
    // tsc pushes each type onto `awaitedTypeStack` to stop mutually recursive
    // thenables (`BadPromiseA`/`BadPromiseB`); a depth cap ends the same cycles.
    const MAX_UNWRAP_DEPTH: usize = 10;
    if depth >= MAX_UNWRAP_DEPTH || matches!(ty, Type::Any) || ty.is_unknown() {
        return ty.clone();
    }

    if let Type::Union(union) = ty {
        return union_type(
            union
                .types()
                .iter()
                .map(|member| awaited_type_at_depth(member, depth + 1))
                .collect(),
        );
    }

    let promised = promise_like_awaited_type(ty);
    if promised != *ty {
        return awaited_type_at_depth(&promised, depth + 1);
    }
    if let Some(promised) = thenable_awaited_type(ty)
        && promised != *ty
    {
        return awaited_type_at_depth(&promised, depth + 1);
    }

    ty.clone()
}

/// The value a user-defined thenable resolves to: the parameter of the callback
/// `then` takes first.
///
/// `await` is erased at parse time, so a promise-typed value is modelled as the
/// value it resolves to everywhere — which is why `Promise<T>` reads as `T`
/// above. A class that implements `Promise<T>` rather than being one
/// (drizzle's `QueryPromise`, and every `PgRaw`/`SQLiteRaw` that extends it) got
/// no such treatment, so `const rows = await db.execute(...)` read as the raw
/// query object and indexing it reported a missing property.
pub(crate) fn thenable_awaited_type(ty: &Type) -> Option<Type> {
    let Type::Object(object) = ty.peeled() else {
        return None;
    };
    let then = object.get_property_access_type("then")?;
    let on_fulfilled = callable_first_parameter(&then)?;
    callable_first_parameter(&on_fulfilled)
}

/// The first parameter of `ty` read as a callable — looking through a union, the
/// way an optional callback parameter is written
/// (`((value: T) => R) | undefined | null`).
fn callable_first_parameter(ty: &Type) -> Option<Type> {
    match ty.peeled() {
        Type::Function(function) => function.parameters().first().cloned(),
        Type::Union(union) => union
            .types()
            .iter()
            .find_map(|member| match member.peeled() {
                Type::Function(function) => function.parameters().first().cloned(),
                _ => None,
            }),
        _ => None,
    }
}

pub(crate) fn check_optional_property_call(
    object: &surge_ts_syntax::ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let object_result = evaluate_expression(object, object_span, symbols, ctx);

    let object_type = match object_result {
        InferredExpression::Known(ty) => ty,
        _ => return None, // already reported by evaluate_expression
    };

    // No early return for a degraded receiver: the `Unknown` arm below walks the
    // arguments, and returning here skipped it — everything inside a callback on
    // an unmodelled receiver (`messages?.map((item) => <article>…</article>)`)
    // went unchecked, which is where trpc's missing UMD-global reports live.
    let base_type = surge_ts_types::remove_nullish(&object_type);
    crate::checks::expr::check_member_accessibility(
        object,
        &base_type,
        property_name,
        property_span,
        false,
        symbols,
        ctx,
    );
    let base_type_name = base_type.name();

    // Same awaited-value modelling as the non-optional path: `result?.catch(...)`
    // on a `Promise<void> | undefined` lands on `void`, not on a missing member.
    if matches!(property_name, "then" | "catch" | "finally")
        && !base_type.is_unknown()
        && !declares_own_property(&base_type, property_name)
    {
        let continued = if property_name == "then" {
            check_promise_then_call(base_type, arguments, symbols, ctx)
        } else {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            Some(base_type)
        };
        return continued.map(|ty| union_type(vec![ty, Type::Undefined]));
    }

    match base_type {
        Type::Any => {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            Some(Type::Any)
        }
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => {
            evaluate_arguments_context_free(object, arguments, symbols, ctx);
            None
        },
        Type::Array(element_type) if property_name == "map" => check_array_map_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        Type::Array(element_type) if property_name == "find" => check_array_find_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if matches!(ty, Type::Undefined | Type::Null) {
                    result_types.push(Type::Undefined);
                    continue;
                }

                if property_name == "map"
                    && let Type::Array(element_type) = ty
                {
                    let mapped = check_array_map_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(mapped);
                    continue;
                }

                let Some(property_type) = ty.get_property_access_type(property_name) else {
                    // A member whose reference peels to the sentinel is a shape
                    // surge could not reconstruct, not a type without the member.
                    if ty.peeled().is_unknown()
                        || crate::checks::expr::carries_leaked_type_parameter(ty, ctx)
                    {
                        return None;
                    }
                    if no_lib_array_member(ty, ctx) {
                        result_types.push(Type::Any);
                        continue;
                    }
                    let diagnostic =
                        Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                let property_type_base = surge_ts_types::remove_nullish(&property_type);

                let declared_member = property_type_base.clone();
                match callable_property_signature(property_type_base) {
                    Type::Function(function_type) => {
                        let function_type = instantiate_declared_member_signature(
                            &function_type,
                            Some(&declared_member),
                            type_arguments,
                            property_span,
                            arguments,
                            expected_return_type,
                            symbols,
                            ctx,
                        );
                        let return_type = check_function_type_call(
                            &function_type,
                            property_span,
                            call_span,
                            type_arguments,
                            arguments,
                            symbols,
                            ctx,
                        )?;
                        result_types.push(return_type);
                    }
                    Type::Any => result_types.push(Type::Any),
                    Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => return None,
                    _ => {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts2349(ctx.file_name.clone()),
                            crate::spans::choose_span(
                                property_span,
                                crate::spans::choose_span(call_span, object_span),
                            ),
                        ));
                        return None;
                    }
                }
            }

            Some(surge_ts_types::union_type(vec![
                surge_ts_types::union_type(result_types),
                Type::Undefined,
            ]))
        }
        _ => {
            if property_name == "map"
                && let Type::Array(element_type) = &base_type
            {
                return check_array_map_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                )
                .map(|ret| surge_ts_types::union_type(vec![ret, Type::Undefined]));
            }

            if property_name == "find"
                && let Type::Array(element_type) = &base_type
            {
                return check_array_find_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                )
                .map(|ret| surge_ts_types::union_type(vec![ret, Type::Undefined]));
            }

            let Some(property_type) = base_type.get_property_access_type(property_name) else {
                if no_lib_array_member(&base_type, ctx) {
                    return Some(surge_ts_types::union_type(vec![Type::Any, Type::Undefined]));
                }
                // Same rule as the plain-call arms: a reference that peels to the
                // sentinel is a shape surge could not reconstruct, not a type
                // without the member.
                if base_type.peeled().is_unknown()
                    || crate::checks::expr::carries_leaked_type_parameter(&base_type, ctx)
                {
                    return None;
                }
                let diagnostic = match crate::checks::expr::property_spelling_suggestion(
                    property_name,
                    &base_type,
                ) {
                    Some(suggestion) => Diagnostic::ts2551(
                        property_name,
                        &base_type_name,
                        suggestion,
                        ctx.file_name.clone(),
                    ),
                    None => Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone()),
                };
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            let property_type_base = surge_ts_types::remove_nullish(&property_type);

            let declared_member = property_type_base.clone();
            match callable_property_signature(property_type_base) {
                Type::Function(function_type) => {
                    let function_type = instantiate_declared_member_signature(
                        &function_type,
                        Some(&declared_member),
                        type_arguments,
                        property_span,
                        arguments,
                        expected_return_type,
                        symbols,
                        ctx,
                    );
                    check_function_type_call(
                        &function_type,
                        property_span,
                        call_span,
                        type_arguments,
                        arguments,
                        symbols,
                        ctx,
                    )
                    .map(|ret| union_type(vec![ret, Type::Undefined]))
                }
                Type::Any => {
                    evaluate_arguments_context_free(object, arguments, symbols, ctx);
                    Some(Type::Any)
                }
                Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => {
                    evaluate_arguments_context_free(object, arguments, symbols, ctx);
                    None
                },
                _ => {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2349(ctx.file_name.clone()),
                        crate::spans::choose_span(
                            property_span,
                            crate::spans::choose_span(call_span, object_span),
                        ),
                    ));
                    None
                }
            }
        }
    }
}

/// A member typed by `typeof fn` over a generic declaration carries that
/// declaration (see the `typeof` arm of `resolve_parsed_type`); the call
/// instantiates its type parameters from the arguments exactly as a call on the
/// declared symbol would, so `vi.fn()` binds `T` (to its default) instead of
/// returning `Mock<T>` with the parameter bare. Any other function-typed member
/// is used as resolved.
fn instantiate_declared_member_signature<'a>(
    function_type: &'a surge_ts_types::FunctionType,
    declared_member: Option<&Type>,
    type_arguments: &[ParsedType],
    type_argument_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> std::borrow::Cow<'a, surge_ts_types::FunctionType> {
    if let Some(member) = function_type
        .declaration()
        .and_then(|declaration| declaration.downcast_ref::<super::DeclaredMemberSignature>())
    {
        // A written signature that names something the capture did not bind
        // (an enclosing parameter the body was resolved under by scope, not by
        // substitution) reports it here as an unresolved name. That report is
        // about the recovery, not the source: drop it and keep the resolved
        // handle, exactly as an unattached signature behaves.
        let diagnostics_before = ctx.diagnostics().len();
        let instantiated = surge_ts_types::with_type_copy_reason(
            surge_ts_types::TypeCopyReason::CallResolution,
            || {
                super::instantiate::instantiate_function_type(
                    function_type,
                    Some(&member.signature),
                    &member.outer_type_arguments,
                    type_arguments,
                    type_argument_span,
                    arguments,
                    expected_return_type,
                    symbols,
                    ctx,
                )
            },
        );
        if ctx.diagnostics().len() != diagnostics_before {
            ctx.truncate_diagnostics(diagnostics_before);
            return std::borrow::Cow::Borrowed(function_type);
        }
        return instantiated;
    }
    let declared_signature = function_type
        .declaration()
        .and_then(|declaration| declaration.downcast_ref::<crate::symbols::FunctionSignatureInfo>())
        .filter(|signature| !signature.type_parameters.is_empty());
    if let Some(signature) = declared_signature {
        return surge_ts_types::with_type_copy_reason(
            surge_ts_types::TypeCopyReason::CallResolution,
            || {
                super::instantiate::instantiate_function_type(
                    function_type,
                    Some(signature),
                    &[],
                    type_arguments,
                    type_argument_span,
                    arguments,
                    expected_return_type,
                    symbols,
                    ctx,
                )
            },
        );
    }
    // A member typed by an interface or alias carrying a generic call signature
    // has no declaration on its handle at all: `resolve_function_type` erases
    // the signature's type parameters to the degradation sentinel and keeps
    // only their rendering. The written signature is read back off the
    // declaration the member names, the same recovery the bare-call path runs.
    let Some(written) = declared_member
        .filter(|_| super::written_call_signature_recovery_enabled())
        .and_then(|declared| super::interface_call_signature_info(declared, ctx))
    else {
        return std::borrow::Cow::Borrowed(function_type);
    };
    surge_ts_types::with_type_copy_reason(surge_ts_types::TypeCopyReason::CallResolution, || {
        super::instantiate::instantiate_function_type(
            function_type,
            Some(&written.signature),
            &written.outer_type_arguments,
            type_arguments,
            type_argument_span,
            arguments,
            expected_return_type,
            symbols,
            ctx,
        )
    })
}

/// The return type of calling a callable member, instantiated. The inference
/// side reads a member's return type straight off the resolved handle, which for
/// a generic call signature still carries the erased type parameters — an object
/// literal holding a nested builder call (`t.router({ post: t.router({…}) })`)
/// is inferred there, so without this the outer call sees an unresolved argument
/// and abandons its own instantiation.
pub(crate) fn callable_member_call_return_type(
    member_type: &Type,
    type_arguments: &[ParsedType],
    property_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Function(declared) = callable_property_signature(member_type.clone()) else {
        return None;
    };
    let function_type = instantiate_declared_member_signature(
        &declared,
        Some(member_type),
        type_arguments,
        property_span,
        arguments,
        None,
        symbols,
        ctx,
    );
    Some(
        super::select_overload_return_type_for_inferred_call(&declared, arguments, symbols, ctx)
            .unwrap_or_else(|| function_type.return_type().clone()),
    )
}

/// Under `noLib` the array member surface comes from the configured replacement
/// lib (roblox-ts's `Array` adds `size`/`push`/`pop`/… absent from the standard
/// JS array surface). surge collapses that interface to `Type::Array` for
/// assignability, discarding its member set, so a method the std surface does not
/// know is not a real typo here — resolve it permissively instead of emitting
/// TS2339. Without `noLib` the std array surface is authoritative.
/// A property typed as a callable object — a type literal carrying call
/// signatures, like expect-type's `toEqualTypeOf: { <E>(v: E): true; <E>(): true }`
/// — is invoked like a function. Surface its call signature so the property-call
/// match treats it as callable instead of a false TS2349.
fn callable_property_signature(ty: Type) -> Type {
    let signature = match &ty {
        Type::Object(object) => object.call_signature().cloned(),
        Type::Reference(_) => match ty.peeled() {
            Type::Object(object) => object.call_signature().cloned(),
            // A reference resolving to a function IS the callable — surface it
            // like the object call-signature case, or the caller's variant
            // match sees the unpeeled reference and misreports TS2349 (lazy
            // value annotations wrap plain function-typed `declare const`s).
            Type::Function(function) => Some(function),
            _ => None,
        },
        _ => None,
    };
    match signature {
        Some(signature) => Type::Function(signature),
        None => ty,
    }
}

fn no_lib_array_member(object_type: &Type, ctx: &CheckerContext) -> bool {
    ctx.options.no_lib && matches!(object_type, Type::Array(_))
}
