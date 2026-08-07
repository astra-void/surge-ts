use super::*;

pub(crate) fn evaluate_expression(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_expression_check();
    match expression {
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let inferred_expression = infer_expression(expression, symbols, ctx);

            for element in elements {
                let _ = evaluate_expression(
                    &element.expression,
                    element.span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }

            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    inferred_expression.clone()
                }),
                fallback_span,
                symbols,
                ctx,
            );
            inferred_expression
        }
        ParsedExpression::Call {
            callee_name,
            callee_span,
            type_arguments,
            arguments,
            ..
        } => match check_call_like(
            callee_name,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::New {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match check_new_like(
            callee,
            *callee_span,
            None,
            type_arguments,
            arguments,
            None,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::PropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            call_span,
            type_arguments,
            arguments,
            ..
        } => match check_property_call_like(
            object,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalPropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            call_span,
            type_arguments,
            arguments,
        } => match check_optional_property_call(
            object,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalCall {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match check_optional_call_like(
            callee,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::NullishCoalescing {
            left,
            left_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            match (left_result, right_result) {
                (InferredExpression::Known(left_type), InferredExpression::Known(right_type)) => {
                    if left_type == Type::Any || left_type.is_unknown() {
                        InferredExpression::Known(left_type)
                    } else if left_type == Type::Undefined {
                        InferredExpression::Known(right_type)
                    } else {
                        let filtered_left = surge_ts_types::remove_nullish(&left_type);
                        InferredExpression::Known(union_type(vec![filtered_left, right_type]))
                    }
                }
                (
                    InferredExpression::Known(Type::Unknown)
                    | InferredExpression::Known(Type::GenuineUnknown)
                    | InferredExpression::Unknown,
                    _,
                )
                | (
                    _,
                    InferredExpression::Known(Type::Unknown)
                    | InferredExpression::Known(Type::GenuineUnknown)
                    | InferredExpression::Unknown,
                ) => InferredExpression::Unknown,
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span: _,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            // `a && b` only evaluates `b` when `a` is truthy, so narrow `b` by the
            // `a` guard: a structured guard (`x.kind === "k" && x.k`, `"p" in x &&
            // x.p`) plus each identifier/property the `&&` chain proves non-nullish
            // (`a.b && a.b > c`).
            let narrowed = matches!(operator, surge_ts_syntax::ParsedLogicalOperator::And)
                .then(|| crate::checks::function::narrow_truthy_operand_symbol_table(left, symbols))
                .flatten();
            let right_result = evaluate_expression(
                right,
                right_span.or(fallback_span),
                narrowed.as_ref().unwrap_or(symbols),
                ctx,
            );

            ops::evaluate_logical_expression(*operator, left_result, right_result)
        }
        ParsedExpression::Binary {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            ops::evaluate_binary_expression(
                left_result,
                right_result,
                *operator,
                *left_span,
                *operator_span,
                *right_span,
                fallback_span,
                ctx,
            )
        }
        ParsedExpression::Unary {
            operator,
            operand,
            operand_span,
            ..
        } => {
            let operand_result =
                evaluate_expression(operand, operand_span.or(fallback_span), symbols, ctx);

            ops::evaluate_unary_expression(
                *operator,
                operand_result,
                operand_span.or(fallback_span),
                ctx,
            )
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
        } => {
            let condition_result =
                evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
            // Narrow a discriminated union per branch so `x.kind === "a" ? x.a :
            // x.b` checks `x.a` against the `"a"` member only.
            let true_symbols =
                crate::checks::function::narrow_condition_symbol_table(condition, symbols, true);
            let false_symbols =
                crate::checks::function::narrow_condition_symbol_table(condition, symbols, false);
            let true_result = evaluate_expression(
                when_true,
                when_true_span.or(fallback_span),
                true_symbols.as_ref().unwrap_or(symbols),
                ctx,
            );
            let false_result = evaluate_expression(
                when_false,
                when_false_span.or(fallback_span),
                false_symbols.as_ref().unwrap_or(symbols),
                ctx,
            );

            ops::evaluate_conditional_expression(condition_result, true_result, false_result)
        }
        ParsedExpression::OptionalPropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
            is_bracketed,
        } => {
            let _ = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
            let inferred_expression = infer_expression(expression, symbols, ctx);
            if let InferredExpression::MissingProperty {
                property_name,
                object_type,
                span,
            } = &inferred_expression
            {
                let diagnostic = missing_property_diagnostic(
                    property_name,
                    object_type,
                    symbols,
                    ctx.file_name.clone(),
                );
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(*span, fallback_span),
                ));
            }
            if !is_bracketed && matches!(inferred_expression, InferredExpression::Known(_)) {
                maybe_emit_index_signature_access(
                    object,
                    property_name,
                    *property_span,
                    fallback_span,
                    symbols,
                    ctx,
                );
            }
            inferred_expression
        }
        ParsedExpression::OptionalIndexAccess {
            object,
            object_span,
            index,
            index_span,
        } => evaluate_optional_index_access(
            object,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => evaluate_index_access(
            object_name,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::SatisfiesExpression {
            expression: satisfied_expression,
            span,
            target_type,
            target_span: _,
        } => {
            let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
            let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
            let resolved_target_type =
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    crate::infer::map_parsed_type(target_type.clone(), ctx)
                });
            ctx.symbols = saved_symbols;

            // Evaluate the left expression contextually against the target type
            // This pushes contextual diagnostics (like excess properties, missing properties).
            let contextual_inferred =
                crate::checks::expected::evaluate_expression_with_expected_type(
                    satisfied_expression,
                    span.or(fallback_span),
                    Some(&resolved_target_type),
                    crate::checks::expected::ExpectedTypeDiagnostic::SatisfiesNotAssignable,
                    symbols,
                    ctx,
                );

            // We must also perform a top-level assignability check for things that don't do it
            // contextually (e.g. primitives, identifiers). However, if contextual checking already
            // failed and returned Unknown, we might get false cascades. Let's do a clean check
            // against the original inferred type.
            let original_inferred =
                crate::infer::infer_expression(satisfied_expression, symbols, ctx);

            // Check if contextual check already failed (meaning it returned Unknown when actual wasn't Unknown).
            let contextual_failed = matches!(
                contextual_inferred,
                crate::infer::InferredExpression::Unknown
            );
            let mut top_level_failed = false;

            if let crate::infer::InferredExpression::Known(actual_type) = &original_inferred {
                if *actual_type != surge_ts_types::Type::Unknown
                    && resolved_target_type != surge_ts_types::Type::Unknown
                {
                    let needs_top_level_check = !matches!(
                        satisfied_expression.as_ref(),
                        surge_ts_syntax::ParsedExpression::ObjectLiteral { .. }
                            | surge_ts_syntax::ParsedExpression::ArrayLiteral { .. }
                            | surge_ts_syntax::ParsedExpression::ConstAssertion { .. }
                            | surge_ts_syntax::ParsedExpression::Conditional { .. }
                    );

                    if needs_top_level_check
                        && !surge_ts_types::is_assignable_to(actual_type, &resolved_target_type)
                    {
                        top_level_failed = true;
                        let actual_type_name = actual_type.name();
                        let target_type_name = resolved_target_type.name();
                        let diagnostic = surge_ts_diagnostics::Diagnostic::ts1360(
                            &actual_type_name,
                            &target_type_name,
                            ctx.file_name.clone(),
                        );
                        let diagnostic = match span.or(fallback_span) {
                            Some(span) => diagnostic.with_span(crate::context::convert_span(span)),
                            None => diagnostic,
                        };
                        ctx.push(diagnostic);
                    }
                }
            }

            let final_inferred = match original_inferred {
                crate::infer::InferredExpression::Known(ty) => {
                    match ctx.options.diagnostic_profile {
                        crate::context::DiagnosticProfile::Tsc => {
                            if let ParsedExpression::ConstAssertion {
                                expression: const_inner,
                                span: const_span,
                            } = &**satisfied_expression
                            {
                                // `x as const satisfies T` keeps the
                                // const-asserted literal type; `infer_expression`
                                // widens literal members. Re-derive it with the
                                // const-aware evaluator, gated to pure literal
                                // trees so the re-evaluation cannot re-emit
                                // expression diagnostics.
                                if is_pure_literal_tree(const_inner) {
                                    match evaluate_const_expression(
                                        const_inner,
                                        const_span.or(fallback_span),
                                        symbols,
                                        ctx,
                                    ) {
                                        crate::infer::InferredExpression::Known(const_ty) => {
                                            crate::infer::InferredExpression::Known(const_ty)
                                        }
                                        _ => crate::infer::InferredExpression::Known(ty),
                                    }
                                } else {
                                    crate::infer::InferredExpression::Known(ty)
                                }
                            } else {
                                crate::infer::InferredExpression::Known(widen_type(&ty))
                            }
                        }
                        crate::context::DiagnosticProfile::Native => {
                            crate::infer::InferredExpression::Known(ty)
                        }
                    }
                }
                other => other,
            };

            if contextual_failed {
                crate::infer::InferredExpression::Unknown
            } else if top_level_failed {
                match ctx.options.diagnostic_profile {
                    crate::context::DiagnosticProfile::Tsc => final_inferred,
                    crate::context::DiagnosticProfile::Native => {
                        crate::infer::InferredExpression::Unknown
                    }
                }
            } else {
                final_inferred
            }
        }
        ParsedExpression::TypeAssertion {
            expression: asserted_expression,
            expression_span,
            ty,
            type_span: _,
        } => {
            // Evaluate the inner expression so it participates in checking
            let _ = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );

            let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
            let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
            let resolved_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                crate::infer::map_parsed_type(ty.clone(), ctx)
            });
            ctx.symbols = saved_symbols;

            // If the type is unresolved (e.g. unknown named type), map_parsed_type
            // already emits TS2304 and returns Type::Unknown.
            // We just return it as the assertion result.
            InferredExpression::Known(resolved_type)
        }
        ParsedExpression::ConstAssertion {
            expression: asserted_expression,
            span: expression_span,
        } => evaluate_const_expression(
            asserted_expression,
            expression_span.or(fallback_span),
            symbols,
            ctx,
        ),
        ParsedExpression::ArrowFunction(arrow_function) => {
            let function_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                check_arrow_function_expression(arrow_function.as_ref().clone(), symbols, ctx)
            });
            InferredExpression::Known(Type::Function(function_type))
        }
        ParsedExpression::NonNullAssertion {
            expression: asserted_expression,
            span: expression_span,
            in_optional_chain,
        } => {
            let inferred = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );

            match inferred {
                InferredExpression::Known(ty) => {
                    let filtered = surge_ts_types::remove_undefined(&ty);
                    if *in_optional_chain {
                        InferredExpression::Known(surge_ts_types::union_type(vec![
                            filtered,
                            surge_ts_types::Type::Undefined,
                        ]))
                    } else {
                        InferredExpression::Known(filtered)
                    }
                }
                other => other,
            }
        }
        ParsedExpression::JsxElement {
            tag_name,
            tag_name_span,
            component_name,
            component_span,
            attributes,
            children,
            span,
        } => {
            crate::checks::jsx::check_jsx_element(
                tag_name,
                *tag_name_span,
                component_name.as_deref(),
                *component_span,
                *span,
                attributes,
                children,
                fallback_span,
                symbols,
                ctx,
            );

            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::JsxFragment { children, .. } => {
            for child in children {
                evaluate_jsx_child(child, fallback_span, symbols, ctx);
            }

            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::PropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
            is_bracketed,
            ..
        } => {
            maybe_emit_unknown_property_receiver(object, *object_span, fallback_span, symbols, ctx);
            let inferred_expression = infer_expression(expression, symbols, ctx);
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    inferred_expression.clone()
                }),
                fallback_span,
                symbols,
                ctx,
            );
            if !is_bracketed && matches!(inferred_expression, InferredExpression::Known(_)) {
                maybe_emit_index_signature_access(
                    object,
                    property_name,
                    *property_span,
                    fallback_span,
                    symbols,
                    ctx,
                );
            }
            inferred_expression
        }
        _ => {
            let inferred_expression = infer_expression(expression, symbols, ctx);
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    inferred_expression.clone()
                }),
                fallback_span,
                symbols,
                ctx,
            );
            inferred_expression
        }
    }
}

/// Walks a JSX child for ordinary diagnostics. Text is inert; `{expression}`
/// containers and nested elements are evaluated so diagnostics such as unresolved
/// names inside `{...}` are still reported.
fn evaluate_jsx_child(
    child: &ParsedJsxChild,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    match child {
        ParsedJsxChild::Text => {}
        ParsedJsxChild::Expression { expression, span } => {
            if let Some(expression) = expression {
                let _ = evaluate_expression(expression, span.or(fallback_span), symbols, ctx);
            }
        }
        ParsedJsxChild::Element(element) => {
            let _ = evaluate_expression(element, fallback_span, symbols, ctx);
        }
    }
}

/// Whether an expression is a tree of literals and array/object literals only —
/// re-evaluating such a tree cannot resolve names or push diagnostics, so the
/// `satisfies` result path may safely re-derive its const-asserted type.
fn is_pure_literal_tree(expression: &ParsedExpression) -> bool {
    match expression {
        ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BooleanLiteral(_) => true,
        ParsedExpression::ArrayLiteral { elements, .. } => elements
            .iter()
            .all(|element| is_pure_literal_tree(&element.expression)),
        ParsedExpression::ObjectLiteral { properties, .. } => properties
            .iter()
            .all(|property| !property.is_spread && !property.is_method && is_pure_literal_tree(&property.value)),
        _ => false,
    }
}
