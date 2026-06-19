use std::collections::BTreeMap;
use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedObjectProperty, TextSpan as SyntaxTextSpan};
use surge_ts_types::{ObjectProperty, Type, is_assignable_to};

use super::expr::{evaluate_expression, source_display_name};
use super::function::check_arrow_function_expression_with_expected_type;
use crate::arena::alloc_object_type;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::program::{
    record_assignability_check, record_object_literal_property_check, record_program_timing,
};
use crate::spans::{choose_span, diagnostic_with_syntax_span};
use crate::symbols::SymbolTable;
use surge_ts_types::{TypeCopyReason, with_type_copy_reason};

#[derive(Clone, Copy)]
pub(crate) enum ExpectedTypeDiagnostic {
    TypeNotAssignable,
    ArgumentNotAssignable,
    SatisfiesNotAssignable,
}

pub(crate) fn evaluate_expression_with_expected_type(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    expected_type: Option<&Type>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    evaluate_expression_with_expected_type_anchored(
        expression,
        fallback_span,
        None,
        expected_type,
        expected_diagnostic,
        symbols,
        ctx,
    )
}

/// Like [`evaluate_expression_with_expected_type`], but threads a `target_span`
/// that whole-value assignability diagnostics (TS2741 missing property and the
/// top-level object mismatch) anchor on, matching tsc which points such errors at
/// the assignment target (e.g. the declaration name) rather than the value. When
/// `target_span` is `None` the behavior is identical to the unanchored entry.
pub(crate) fn evaluate_expression_with_expected_type_anchored(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_type: Option<&Type>,
    _expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(expected_type) = expected_type else {
        return evaluate_expression(expression, fallback_span, symbols, ctx);
    };

    // A generic expected type (`Props`, `Box<T>`, …) is a nominal
    // `Type::Reference`; peel it to its structural shape so the contextual-typing
    // dispatch below (function/tuple/array/object/union) sees the real form
    // instead of falling through to context-free evaluation.
    let peeled_expected;
    let expected_type = match expected_type {
        Type::Reference(reference) => {
            peeled_expected = reference.resolve().peeled();
            &peeled_expected
        }
        other => other,
    };

    if let (Type::Function(expected_function_type), ParsedExpression::ArrowFunction(arrow)) =
        (expected_type, expression)
    {
        let function_type = check_arrow_function_expression_with_expected_type(
            with_type_copy_reason(TypeCopyReason::ExpectedType, || arrow.as_ref().clone()),
            Some(expected_function_type),
            symbols,
            ctx,
        );
        return InferredExpression::Known(Type::Function(function_type));
    }

    if let ParsedExpression::ConstAssertion {
        expression: inner, ..
    } = expression
    {
        return evaluate_expression_with_expected_type_anchored(
            inner,
            fallback_span,
            target_span,
            Some(expected_type),
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    if matches!(expression, ParsedExpression::Conditional { .. }) {
        return evaluate_conditional_expression_with_expected_type(
            expression,
            fallback_span,
            expected_type,
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    if let (Type::Tuple(expected_elements), ParsedExpression::ArrayLiteral { elements, span }) =
        (expected_type, expression)
    {
        return evaluate_tuple_literal_with_expected_type(
            elements,
            expected_elements,
            choose_span(*span, fallback_span),
            symbols,
            ctx,
        );
    }

    if let (Type::Array(expected_element_type), ParsedExpression::ArrayLiteral { elements, span }) =
        (expected_type, expression)
    {
        return evaluate_array_literal_with_expected_type(
            elements,
            expected_element_type,
            choose_span(*span, fallback_span),
            symbols,
            ctx,
        );
    }

    if let (
        Type::Object(expected_object_type),
        ParsedExpression::ObjectLiteral { properties, span },
    ) = (expected_type, expression)
    {
        return evaluate_object_literal_with_expected_type(
            properties,
            expected_object_type,
            choose_span(*span, fallback_span),
            target_span,
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    // Contextual typing through a union: when the expected type is a union whose
    // only non-nullish member is a single concrete type (e.g. `{ ... } | null`,
    // mapped here to `{ ... } | undefined`), use that member as the contextual
    // type so an object/array literal's property values are evaluated with the
    // expected element types rather than context-free (which would widen
    // member-access values like `res.status` toward `unknown`).
    if let Type::Union(union) = expected_type {
        let mut non_nullish = union
            .types()
            .iter()
            .filter(|member| !matches!(member, Type::Undefined | Type::Void));
        if let (Some(member), None) = (non_nullish.next(), non_nullish.next()) {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }
    }

    evaluate_expression(expression, fallback_span, symbols, ctx)
}

fn evaluate_array_literal_with_expected_type(
    elements: &[surge_ts_syntax::ParsedArrayElement],
    expected_element_type: &Type,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    for element in elements {
        let inferred_element = evaluate_expression_with_expected_type(
            &element.expression,
            element.span,
            Some(expected_element_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_element {
            InferredExpression::Known(actual_type) => {
                if actual_type == Type::Unknown {
                    continue;
                }

                if !is_assignable_to(&actual_type, expected_element_type) {
                    let actual_type_name = actual_type.name();
                    let expected_type_name = expected_element_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(element.span, fallback_span),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
        }
    }

    InferredExpression::Known(Type::Array(Box::new(with_type_copy_reason(
        TypeCopyReason::ExpectedType,
        || expected_element_type.clone(),
    ))))
}

fn evaluate_tuple_literal_with_expected_type(
    elements: &[surge_ts_syntax::ParsedArrayElement],
    expected_elements: &[Type],
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    for (index, element) in elements.iter().enumerate() {
        if index >= expected_elements.len() {
            let source_type_name = Type::Array(Box::new(Type::Unknown)).name();
            let target_type_name = Type::Tuple(expected_elements.to_vec()).name();
            let diagnostic =
                Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                choose_span(element.span, fallback_span),
            ));
            return InferredExpression::Unknown;
        }

        let expected_element_type = &expected_elements[index];
        let inferred_element = evaluate_expression_with_expected_type(
            &element.expression,
            element.span,
            Some(expected_element_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_element {
            InferredExpression::Known(actual_type) => {
                if actual_type == Type::Unknown {
                    continue;
                }

                if !is_assignable_to(&actual_type, expected_element_type) {
                    let actual_type_name = actual_type.name();
                    let expected_type_name = expected_element_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(element.span, fallback_span),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
        }
    }

    if elements.len() != expected_elements.len() {
        let source_type_name = Type::Array(Box::new(Type::Unknown)).name();
        let target_type_name = Type::Tuple(expected_elements.to_vec()).name();
        let diagnostic =
            Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

        ctx.push(diagnostic_with_syntax_span(diagnostic, fallback_span));
        return InferredExpression::Unknown;
    }

    InferredExpression::Known(Type::Tuple(expected_elements.to_vec()))
}

fn evaluate_object_literal_with_expected_type(
    properties: &[ParsedObjectProperty],
    expected_object_type: &surge_ts_types::ObjectType,
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_start = Instant::now();
    let mut inferred_property_types = BTreeMap::new();
    // The empty object type `{}` (no properties, no string index) accepts any
    // object literal without excess-property errors, matching tsc. Library
    // signatures like `Object.keys(o: {})` rely on this.
    // A `{ ...source }` spread contributes `source`'s properties under an empty
    // name; it is neither an excess key nor a single checkable property here, and
    // it may supply required properties we don't track by name. Skip spreads in
    // the excess-property scan and, when any spread is present, in the missing
    // required-property scan below (conservative: under-check rather than emit a
    // false `TS2353`/`TS2741`).
    let has_spread = properties.iter().any(|property| property.is_spread);
    if !expected_object_type.allows_string_index_access()
        && !expected_object_type.properties.is_empty()
        && let Some(property) = properties
            .iter()
            .find(|property| {
                !property.is_spread && !expected_object_type.contains_property(&property.name)
            })
    {
        let diagnostic = Diagnostic::ts2353(
            &property.name,
            &Type::Object(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                expected_object_type.clone()
            }))
            .name(),
            ctx.file_name.clone(),
        );

        ctx.push(diagnostic_with_syntax_span(
            diagnostic,
            choose_span(
                property.name_span,
                choose_span(property.span, fallback_span),
            ),
        ));
        return InferredExpression::Unknown;
    }

    for property in properties {
        if property.is_spread {
            continue;
        }
        record_object_literal_property_check();
        let expected_property = if let Some(expected_property) =
            expected_object_type.get_property(&property.name).cloned()
        {
            expected_property
        } else if let Some(index_type) = expected_object_type.string_index_type.as_deref().cloned()
        {
            ObjectProperty::required(index_type)
        } else {
            continue;
        };

        let contextual_property_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
            expected_property.ty.clone()
        });
        let expected_property_type = if expected_property.is_optional() {
            surge_ts_types::union_type(vec![
                with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                    expected_property.ty.clone()
                }),
                Type::Undefined,
            ])
        } else {
            with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                expected_property.ty.clone()
            })
        };

        let inferred_property = evaluate_expression_with_expected_type(
            &property.value,
            property.value_span.or(property.span),
            Some(&contextual_property_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_property {
            InferredExpression::Known(actual_type) => {
                if actual_type == Type::Unknown {
                    inferred_property_types.insert(property.name.clone(), Type::Unknown);
                    continue;
                }

                inferred_property_types.insert(
                    property.name.clone(),
                    with_type_copy_reason(TypeCopyReason::ExpectedType, || actual_type.clone()),
                );
                record_assignability_check();
                if !is_assignable_to(&actual_type, &expected_property_type) {
                    let actual_type_name =
                        source_display_name(&actual_type, &expected_property_type);
                    let expected_type_name = expected_property_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(
                            property.name_span,
                            choose_span(
                                property.value_span,
                                choose_span(property.span, fallback_span),
                            ),
                        ),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                inferred_property_types.insert(property.name.clone(), Type::Unknown);
                return InferredExpression::Unknown;
            }
        }
    }

    if let Some((property_name, _)) = (!has_spread)
        .then(|| {
            expected_object_type
                .required_properties()
                .find(|(property_name, _)| {
                    !properties
                        .iter()
                        .any(|property| property.name == property_name.as_str())
                })
        })
        .flatten()
    {
        let source_type_name = crate::checks::expr::widen_type(&object_literal_source_type_name(
            properties,
            &inferred_property_types,
        ))
        .name();
        let target_type_name =
            Type::Object(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                expected_object_type.clone()
            }))
            .name();

        // tsc surfaces a missing required property differently for an
        // intersection target: it reports the outer assignability code (the
        // missing property becomes nested elaboration) rather than the
        // standalone TS2741. Mirror that so the reported code matches.
        let diagnostic = if expected_object_type.is_intersection {
            match expected_diagnostic {
                ExpectedTypeDiagnostic::TypeNotAssignable => {
                    Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                ExpectedTypeDiagnostic::ArgumentNotAssignable => {
                    Diagnostic::ts2345(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                ExpectedTypeDiagnostic::SatisfiesNotAssignable => {
                    Diagnostic::ts1360(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
            }
        } else {
            match expected_diagnostic {
                ExpectedTypeDiagnostic::SatisfiesNotAssignable => {
                    Diagnostic::ts1360(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                _ => Diagnostic::ts2741(
                    property_name,
                    &source_type_name,
                    &target_type_name,
                    ctx.file_name.clone(),
                ),
            }
        };

        ctx.push(diagnostic_with_syntax_span(
            diagnostic,
            choose_span(target_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    }

    let result = InferredExpression::Known(Type::Object(with_type_copy_reason(
        TypeCopyReason::ExpectedType,
        || expected_object_type.clone(),
    )));
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.object_literal_checking += object_start.elapsed()
    });
    result
}

fn object_literal_source_type_name(
    properties: &[ParsedObjectProperty],
    inferred_property_types: &BTreeMap<String, Type>,
) -> Type {
    let properties = properties
        .iter()
        .map(|property| {
            let ty = inferred_property_types
                .get(&property.name)
                .cloned()
                .unwrap_or(Type::Unknown);
            (property.name.clone(), ObjectProperty::required(ty))
        })
        .collect::<surge_ts_types::PropertyMap>();

    Type::Object(alloc_object_type(properties, None))
}

fn evaluate_conditional_expression_with_expected_type(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    expected_type: &Type,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let ParsedExpression::Conditional {
        condition,
        condition_span,
        when_true,
        when_true_span,
        when_false,
        when_false_span,
    } = expression
    else {
        return evaluate_expression(expression, fallback_span, symbols, ctx);
    };

    // Narrow a discriminated union for each branch (`x.kind === "a" ? … : …`).
    let true_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, true);
    let false_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, false);
    let true_symbols = true_symbols.as_ref().unwrap_or(symbols);
    let false_symbols = false_symbols.as_ref().unwrap_or(symbols);

    if *expected_type == Type::Any {
        let _ = evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
        let _ = evaluate_expression(when_true, when_true_span.or(fallback_span), true_symbols, ctx);
        let _ =
            evaluate_expression(when_false, when_false_span.or(fallback_span), false_symbols, ctx);

        return InferredExpression::Known(Type::Any);
    }

    let condition_result =
        evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
    let true_result = evaluate_expression_with_expected_type(
        when_true,
        when_true_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        true_symbols,
        ctx,
    );
    let false_result = evaluate_expression_with_expected_type(
        when_false,
        when_false_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        false_symbols,
        ctx,
    );

    let true_branch_span = when_true_span.or(fallback_span);
    let false_branch_span = when_false_span.or(fallback_span);
    let mut has_contextual_mismatch = false;
    let true_branch_type = known_branch_type(&true_result);
    let false_branch_type = known_branch_type(&false_result);
    let branch_types_differ = match (true_branch_type, false_branch_type) {
        (Some(true_type), Some(false_type)) => {
            match (true_type.base_primitive(), false_type.base_primitive()) {
                (Some(true_base), Some(false_base)) => true_base != false_base,
                _ => true_type != false_type,
            }
        }
        _ => false,
    };

    has_contextual_mismatch |= check_conditional_branch_expected_type(
        true_result,
        true_branch_span,
        expected_type,
        expected_diagnostic,
        ctx,
    );
    if !branch_types_differ || !has_contextual_mismatch {
        has_contextual_mismatch |= check_conditional_branch_expected_type(
            false_result,
            false_branch_span,
            expected_type,
            expected_diagnostic,
            ctx,
        );
    }

    if matches!(condition_result, InferredExpression::Unknown) {
        return InferredExpression::Unknown;
    }

    if has_contextual_mismatch {
        return InferredExpression::Unknown;
    }

    InferredExpression::Known(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
        expected_type.clone()
    }))
}

fn check_conditional_branch_expected_type(
    branch_result: InferredExpression,
    branch_span: Option<SyntaxTextSpan>,
    expected_type: &Type,
    expected_diagnostic: ExpectedTypeDiagnostic,
    ctx: &mut CheckerContext,
) -> bool {
    match branch_result {
        InferredExpression::Known(branch_type) => {
            if branch_type == Type::Unknown {
                return false;
            }

            if is_assignable_to(&branch_type, expected_type) {
                return false;
            }

            push_expected_type_mismatch(
                &branch_type,
                expected_type,
                branch_span,
                expected_diagnostic,
                ctx,
            );
            true
        }
        InferredExpression::UnresolvedIdentifier { .. } => false,
        InferredExpression::MissingProperty { .. } => false,
        InferredExpression::Unknown => false,
    }
}

fn known_branch_type(branch_result: &InferredExpression) -> Option<&Type> {
    match branch_result {
        InferredExpression::Known(ty) if *ty != Type::Unknown => Some(ty),
        _ => None,
    }
}

fn push_expected_type_mismatch(
    source_type: &Type,
    expected_type: &Type,
    span: Option<SyntaxTextSpan>,
    diagnostic_kind: ExpectedTypeDiagnostic,
    ctx: &mut CheckerContext,
) {
    let source_type_name = source_display_name(source_type, expected_type);
    let expected_type_name = expected_type.name();
    let diagnostic = match diagnostic_kind {
        ExpectedTypeDiagnostic::TypeNotAssignable => Diagnostic::ts2322(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
        ExpectedTypeDiagnostic::ArgumentNotAssignable => Diagnostic::ts2345(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
        ExpectedTypeDiagnostic::SatisfiesNotAssignable => Diagnostic::ts1360(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
    };

    ctx.push(diagnostic_with_syntax_span(diagnostic, span));
}
