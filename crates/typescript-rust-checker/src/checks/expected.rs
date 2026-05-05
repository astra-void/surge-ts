use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedExpression, ParsedObjectProperty, TextSpan as SyntaxTextSpan};
use typescript_rust_types::{ObjectProperty, Type, is_assignable_to};

use super::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::spans::{choose_span, diagnostic_with_syntax_span};
use crate::symbols::SymbolTable;

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
    _expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(expected_type) = expected_type else {
        return evaluate_expression(expression, fallback_span, symbols, ctx);
    };

    if let ParsedExpression::ConstAssertion {
        expression: inner, ..
    } = expression
    {
        return evaluate_expression_with_expected_type(
            inner,
            fallback_span,
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
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    evaluate_expression(expression, fallback_span, symbols, ctx)
}

fn evaluate_array_literal_with_expected_type(
    elements: &[typescript_rust_syntax::ParsedArrayElement],
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

    InferredExpression::Known(Type::Array(Box::new(expected_element_type.clone())))
}

fn evaluate_tuple_literal_with_expected_type(
    elements: &[typescript_rust_syntax::ParsedArrayElement],
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
    expected_object_type: &typescript_rust_types::ObjectType,
    fallback_span: Option<SyntaxTextSpan>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    if let Some(property) = properties
        .iter()
        .find(|property| !expected_object_type.contains_property(&property.name))
    {
        let diagnostic = Diagnostic::ts2353(
            &property.name,
            &Type::Object(expected_object_type.clone()).name(),
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
        // Writes and contextual object checking use the declared property type,
        // not the widened access type.
        let Some(expected_property_type) = expected_object_type.get_property_type(&property.name)
        else {
            continue;
        };

        let inferred_property = evaluate_expression_with_expected_type(
            &property.value,
            property.value_span.or(property.span),
            Some(expected_property_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_property {
            InferredExpression::Known(actual_type) => {
                if actual_type == Type::Unknown {
                    continue;
                }

                if !is_assignable_to(&actual_type, expected_property_type) {
                    let actual_type_name = actual_type.name();
                    let expected_type_name = expected_property_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(
                            property.value_span,
                            choose_span(property.span, fallback_span),
                        ),
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

    if let Some((property_name, _)) =
        expected_object_type
            .required_properties()
            .find(|(property_name, _)| {
                !properties
                    .iter()
                    .any(|property| property.name == property_name.as_str())
            })
    {
        let source_type_name = object_literal_source_type_name(properties, symbols).name();
        let target_type_name = Type::Object(expected_object_type.clone()).name();

        let diagnostic = match expected_diagnostic {
            ExpectedTypeDiagnostic::SatisfiesNotAssignable => {
                Diagnostic::ts1360(&source_type_name, &target_type_name, ctx.file_name.clone())
            }
            _ => Diagnostic::ts2741(
                property_name,
                &source_type_name,
                &target_type_name,
                ctx.file_name.clone(),
            ),
        };

        ctx.push(diagnostic_with_syntax_span(diagnostic, fallback_span));
        return InferredExpression::Unknown;
    }

    InferredExpression::Known(Type::Object(expected_object_type.clone()))
}

fn object_literal_source_type_name(
    properties: &[ParsedObjectProperty],
    symbols: &SymbolTable,
) -> Type {
    let properties = properties
        .iter()
        .map(|property| {
            (
                property.name.clone(),
                match infer_expression(&property.value, symbols) {
                    InferredExpression::Known(ty) => ObjectProperty::required(ty),
                    _ => ObjectProperty::required(Type::Unknown),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    Type::Object(typescript_rust_types::ObjectType { properties })
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

    if *expected_type == Type::Any {
        let _ = evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
        let _ = evaluate_expression(when_true, when_true_span.or(fallback_span), symbols, ctx);
        let _ = evaluate_expression(when_false, when_false_span.or(fallback_span), symbols, ctx);

        return InferredExpression::Known(Type::Any);
    }

    let condition_result =
        evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
    let true_result = evaluate_expression_with_expected_type(
        when_true,
        when_true_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        symbols,
        ctx,
    );
    let false_result = evaluate_expression_with_expected_type(
        when_false,
        when_false_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        symbols,
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

    InferredExpression::Known(expected_type.clone())
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
    let source_type_name = source_type.name();
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
