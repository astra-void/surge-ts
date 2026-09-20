use super::*;
use surge_ts_syntax::{ParsedLogicalOperator, ParsedType, ParsedUnaryOperator};

pub(crate) fn evaluate_expression(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_expression_check();
    crate::checks::check_umd_global_value_reference(expression, fallback_span, symbols, ctx);
    match expression {
        // The inference pass types the literal but does not check it, so a
        // property value's own errors — an unresolved name, a bad member, an
        // untyped callback parameter — never surfaced. Same split the array
        // literal arm below uses: infer for the type, evaluate for diagnostics.
        ParsedExpression::ObjectLiteral { properties, .. } => {
            let inferred_expression = infer_expression(expression, symbols, ctx);

            for property in properties {
                // Method and accessor shorthand is checked by the inference pass
                // itself, which must route it through the arrow-checking path to
                // honor its declared signature; evaluating it here as well would
                // duplicate every diagnostic that raised.
                if property.is_method || property.is_accessor {
                    continue;
                }
                if property.is_shorthand {
                    ctx.shorthand_property_depth += 1;
                }
                let property_result = evaluate_expression(
                    &property.value,
                    property.value_span.or(property.span).or(fallback_span),
                    symbols,
                    ctx,
                );
                if property.is_spread {
                    super::check_object_spread_type(
                        &property_result,
                        property.span.or(property.value_span).or(fallback_span),
                        ctx,
                    );
                }
                if property.is_shorthand {
                    ctx.shorthand_property_depth -= 1;
                }
            }

            inferred_expression
        }
        // A template's interpolations are ordinary expressions and carry their own
        // errors (`${process.env.PORT}` is still a TS4111 index-signature access).
        ParsedExpression::TemplateLiteral {
            expressions,
            span,
            expression_spans,
            is_tagged,
            ..
        } => {
            let mut tag_signature = None;
            for (index, interpolation) in expressions.iter().enumerate() {
                let interpolation_span = expression_spans
                    .get(index)
                    .copied()
                    .flatten()
                    .or(*span)
                    .or(fallback_span);
                let result = evaluate_expression(interpolation, interpolation_span, symbols, ctx);
                let InferredExpression::Known(ty) = &result else {
                    continue;
                };
                if *is_tagged && index == 0 {
                    tag_signature = tagged_template_signature(ty);
                    continue;
                }
                // A symbol cannot be converted to a string implicitly (TS2731).
                if type_may_be_symbol(ty) {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2731(ctx.file_name.clone()),
                        interpolation_span,
                    ));
                }
                if let Some(signature) = &tag_signature {
                    // The tag receives the strings array first, so interpolation
                    // `index` is argument `index`. tsc reports a call's first
                    // inapplicable argument and stops.
                    if check_tagged_template_argument(signature, index, ty, interpolation_span, ctx) {
                        tag_signature = None;
                    }
                }
            }
            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let inferred_expression = infer_expression(expression, symbols, ctx);

            for element in elements {
                let element_result = evaluate_expression(
                    &element.expression,
                    element.span.or(fallback_span),
                    symbols,
                    ctx,
                );
                if element.spread {
                    super::check_iterable_operand(&element_result, element.span, true, ctx);
                }
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
            span,
            type_arguments,
            arguments,
        } => match check_new_like(
            callee,
            *callee_span,
            *span,
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
            None,
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
            None,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::ExpressionCall {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match crate::checks::call::check_expression_call(
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
        } => evaluate_nullish_coalescing(
            left,
            left_span,
            right,
            right_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span: _,
            right,
            right_span,
        } => evaluate_logical(
            left,
            left_span,
            operator,
            right,
            right_span,
            fallback_span,
            symbols,
            ctx,
        ),
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
            let binary_span = match (left_span, right_span) {
                (Some(left_span), Some(right_span)) => Some(SyntaxTextSpan {
                    start: left_span.start,
                    end: right_span.end,
                }),
                _ => fallback_span,
            };
            if let Some(always_false) = equality_result_when_unequal(*operator) {
                report_reference_and_nan_equality(left, right, always_false, binary_span, symbols, ctx);
            }
            if matches!(operator, surge_ts_syntax::ParsedBinaryOperator::In) {
                check_in_operands(
                    right,
                    &left_result,
                    &right_result,
                    left_span.or(fallback_span),
                    right_span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }

            ops::evaluate_binary_expression(
                left_result,
                right_result,
                *operator,
                *left_span,
                *operator_span,
                *right_span,
                binary_span,
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

            if matches!(
                operator,
                ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus | ParsedUnaryOperator::BitwiseNot
            ) && let InferredExpression::Known(operand_type) = &operand_result
            {
                check_numeric_unary_operand(
                    *operator,
                    operand,
                    operand_type,
                    operand_span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }
            if *operator == ParsedUnaryOperator::Delete {
                super::check_delete_operand(
                    operand,
                    operand_span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }

            ops::evaluate_unary_expression(*operator, operand_result)
        }
        ParsedExpression::Update {
            operand,
            operand_span,
        } => {
            let operand_result =
                evaluate_expression(operand, operand_span.or(fallback_span), symbols, ctx);

            super::check_update_operand(
                operand,
                operand_span.or(fallback_span),
                &operand_result,
                symbols,
                ctx,
            );

            super::update_result_type(&operand_result)
        }
        ParsedExpression::Await {
            operand,
            operand_span,
        } => {
            let operand_result =
                evaluate_expression(operand, operand_span.or(fallback_span), symbols, ctx);

            match operand_result {
                InferredExpression::Known(ty) => {
                    InferredExpression::Known(crate::checks::call::awaited_type(&ty))
                }
                other => other,
            }
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
            truthiness_tests,
        } => {
            crate::checks::function::report_unreferenced_callable_conditions(
                truthiness_tests,
                symbols,
                ctx,
            );
            evaluate_conditional(
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
            fallback_span,
            symbols,
            ctx,
        )
        }
        ParsedExpression::OptionalPropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
            is_bracketed,
        } => evaluate_optional_property_access(
            object,
            object_span,
            property_name,
            property_span,
            is_bracketed,
            expression,
            fallback_span,
            symbols,
            ctx,
        ),
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
        } => evaluate_satisfies_expression(
            satisfied_expression,
            span,
            target_type,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::TypeAssertion {
            expression: asserted_expression,
            expression_span,
            ty,
            type_span: _,
        } => evaluate_type_assertion(
            asserted_expression,
            expression_span,
            ty,
            fallback_span,
            symbols,
            ctx,
        ),
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
        } => evaluate_non_null_assertion(
            asserted_expression,
            expression_span,
            in_optional_chain,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::JsxElement {
            tag_name,
            tag_name_span,
            component_name,
            component_span,
            attributes,
            children,
            span,
        } => {
            crate::checks::jsx::check_jsx_factory_reference(*tag_name_span, fallback_span, ctx);
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
        ParsedExpression::JsxFragment { children, span } => {
            crate::checks::jsx::check_jsx_factory_reference(*span, fallback_span, ctx);
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
        } => evaluate_property_access(
            object,
            object_span,
            property_name,
            property_span,
            is_bracketed,
            expression,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::ElementAccess {
            object,
            object_span,
            index,
            index_span,
        } => {
            let receiver = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
            check_property_receiver(object, &receiver, *object_span, fallback_span, symbols, ctx);
            if let InferredExpression::Known(receiver_type) = &receiver
                && let Type::Tuple(elements) = receiver_type.peeled()
                && let ParsedExpression::NumberLiteral(value) = index.as_ref()
                && let Ok(index_value) = value.parse::<i64>()
                && usize::try_from(index_value).map_or(true, |index| index >= elements.len())
            {
                super::index_access::report_tuple_index_out_of_bounds(
                    &elements,
                    index_value,
                    index_span.or(*object_span).or(fallback_span),
                    ctx,
                );
                return InferredExpression::Known(Type::Undefined);
            }
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

fn evaluate_nullish_coalescing(
    left: &Box<ParsedExpression>,
    left_span: &Option<SyntaxTextSpan>,
    right: &Box<ParsedExpression>,
    right_span: &Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
    // With no outer contextual type, tsc contextually types the right
    // operand from the left's — that is what gives `opts.uri ?? ((id) =>
    // id)`'s parameter a type instead of a false TS7006.
    let right_result = match contextual_default_operand_type(&left_result) {
        Some(contextual) => match empty_object_fallback_type(right, &contextual) {
            Some(fallback) => InferredExpression::Known(fallback),
            None => crate::checks::expected::evaluate_expression_with_expected_type(
                right,
                right_span.or(fallback_span),
                Some(&contextual),
                crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
                symbols,
                ctx,
            ),
        },
        None => evaluate_expression(right, right_span.or(fallback_span), symbols, ctx),
    };

    match (left_result, right_result) {
        (InferredExpression::Known(left_type), InferredExpression::Known(right_type)) => {
            InferredExpression::Known(crate::infer::expression::nullish_coalescing_result(
                left_type, right_type,
            ))
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

fn evaluate_logical(
    left: &Box<ParsedExpression>,
    left_span: &Option<SyntaxTextSpan>,
    operator: &ParsedLogicalOperator,
    right: &Box<ParsedExpression>,
    right_span: &Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
    // `a && b` only evaluates `b` when `a` is truthy, so narrow `b` by the
    // `a` guard: a structured guard (`x.kind === "k" && x.k`, `"p" in x &&
    // x.p`) plus each identifier/property the `&&` chain proves non-nullish
    // (`a.b && a.b > c`).
    let narrowed = match operator {
        surge_ts_syntax::ParsedLogicalOperator::And => {
            let guarded =
                crate::checks::function::narrow_truthy_operand_symbol_table(left, symbols);
            // That path only knows the syntactic guards; a user-defined
            // predicate call in the chain still has to stop its subject
            // from reading as a genuine-unknown receiver on the right.
            let downgraded = downgrade_predicate_guarded_genuine_unknown(
                left,
                guarded.as_ref().unwrap_or(symbols),
            );
            downgraded.or(guarded)
        }
        // `a || b` only evaluates `b` when `a` is falsy, so the guards
        // `a`'s falsity proves hold on the right (`typeof x !== "object"
        // || !("p" in x) || typeof x.p !== "string"`).
        surge_ts_syntax::ParsedLogicalOperator::Or => {
            crate::checks::function::narrow_falsy_operand_symbol_table(left, symbols)
        }
    };
    // A user-defined predicate in the chain narrows its subject for the
    // right operand too (`ts.isImportDeclaration(n) && n.moduleSpecifier`).
    // `a || b` only evaluates `b` when `a` is falsy, so a negated
    // predicate narrows there the same way (`!isA(x) || x.av`).
    let predicate_branch = match operator {
        surge_ts_syntax::ParsedLogicalOperator::And => true,
        surge_ts_syntax::ParsedLogicalOperator::Or => false,
    };
    let narrowed = crate::checks::function::narrow_predicate_guards_symbol_table(
        left,
        narrowed.as_ref().unwrap_or(symbols),
        predicate_branch,
        ctx,
    )
    .or(narrowed);
    // A guard on an element access (`xs[i] && xs[i].x`) narrows the
    // access itself rather than any binding.
    let narrowed = crate::checks::function::narrow_element_reference_guards_symbol_table(
        left,
        predicate_branch,
        narrowed.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(narrowed);
    let right_symbols = narrowed.as_ref().unwrap_or(symbols);
    // `a || b` hands `b` the same contextual type `a ?? b` does.
    let right_contextual = matches!(operator, surge_ts_syntax::ParsedLogicalOperator::Or)
        .then(|| contextual_default_operand_type(&left_result))
        .flatten();
    let right_result = match right_contextual {
        Some(contextual) => match empty_object_fallback_type(right, &contextual) {
            Some(fallback) => InferredExpression::Known(fallback),
            None => crate::checks::expected::evaluate_expression_with_expected_type(
                right,
                right_span.or(fallback_span),
                Some(&contextual),
                crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
                right_symbols,
                ctx,
            ),
        },
        None => evaluate_expression(right, right_span.or(fallback_span), right_symbols, ctx),
    };

    ops::evaluate_logical_expression(*operator, left_result, right_result)
}

fn evaluate_conditional(
    condition: &Box<ParsedExpression>,
    condition_span: &Option<SyntaxTextSpan>,
    when_true: &Box<ParsedExpression>,
    when_true_span: &Option<SyntaxTextSpan>,
    when_false: &Box<ParsedExpression>,
    when_false_span: &Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let condition_result =
        evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
    // Narrow a discriminated union per branch so `x.kind === "a" ? x.a :
    // x.b` checks `x.a` against the `"a"` member only.
    let true_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, true);
    // The branch where the guard holds also sees a guarded `unknown` as
    // narrowed, exactly as the `if` form does.
    let true_symbols = downgrade_guarded_genuine_unknown(
        condition,
        true_symbols.as_ref().unwrap_or(symbols),
        true,
    )
    .or(true_symbols);
    let true_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        true_symbols.as_ref().unwrap_or(symbols),
        true,
        ctx,
    )
    .or(true_symbols);
    let true_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        true,
        true_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(true_symbols);
    let false_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, false);
    let false_symbols = downgrade_guarded_genuine_unknown(
        condition,
        false_symbols.as_ref().unwrap_or(symbols),
        false,
    )
    .or(false_symbols);
    let false_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        false_symbols.as_ref().unwrap_or(symbols),
        false,
        ctx,
    )
    .or(false_symbols);
    let false_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        false,
        false_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(false_symbols);
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

fn evaluate_optional_property_access(
    object: &Box<ParsedExpression>,
    object_span: &Option<SyntaxTextSpan>,
    property_name: &String,
    property_span: &Option<SyntaxTextSpan>,
    is_bracketed: &bool,
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let receiver = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
    if !*is_bracketed
        && let InferredExpression::Known(receiver_type) = &receiver
    {
        check_member_accessibility(
            object,
            receiver_type,
            property_name,
            *property_span,
            false,
            symbols,
            ctx,
        );
    }
    let inferred_expression = infer_expression(expression, symbols, ctx);
    if *is_bracketed
        && let InferredExpression::MissingProperty { object_type, .. } = &inferred_expression
    {
        report_missing_element(
            property_name,
            &Type::StringLiteral(property_name.clone()),
            object_type,
            *property_span,
            element_access_span(*object_span, *property_span).or(fallback_span),
            symbols,
            ctx,
        );
        return InferredExpression::Unknown;
    }
    if let InferredExpression::MissingProperty {
        property_name,
        object_type,
        span,
    } = &inferred_expression
    {
        let diagnostic =
            missing_property_diagnostic(property_name, object_type, symbols, ctx.file_name.clone());
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

fn evaluate_satisfies_expression(
    satisfied_expression: &Box<ParsedExpression>,
    span: &Option<SyntaxTextSpan>,
    target_type: &ParsedType,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
    let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
    let resolved_target_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
        crate::infer::map_parsed_type(target_type.clone(), ctx)
    });
    ctx.symbols = saved_symbols;

    // Evaluate the left expression contextually against the target type
    // This pushes contextual diagnostics (like excess properties, missing properties).
    let contextual_inferred = crate::checks::expected::evaluate_expression_with_expected_type(
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
    let original_inferred = crate::infer::infer_expression(satisfied_expression, symbols, ctx);

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

            // Either side reaching the degradation sentinel anywhere —
            // not just at the top — means surge failed to model part of
            // the shape, so a mismatch says nothing about the source.
            // Same no-cascade rule the assignment checks apply, and
            // checked only after assignability already failed: the deep
            // walk forces lazy references, which perturbs later
            // resolutions when run on every clean check.
            if needs_top_level_check
                && !surge_ts_types::is_assignable_to(actual_type, &resolved_target_type)
                && !crate::checks::function::type_contains_unknown(actual_type)
                && !crate::checks::function::type_contains_unknown(&resolved_target_type)
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
            crate::context::DiagnosticProfile::Native => crate::infer::InferredExpression::Unknown,
        }
    } else {
        final_inferred
    }
}

fn evaluate_type_assertion(
    asserted_expression: &Box<ParsedExpression>,
    expression_span: &Option<SyntaxTextSpan>,
    ty: &ParsedType,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
    let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
    let resolved_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
        crate::infer::map_parsed_type(ty.clone(), ctx)
    });
    ctx.symbols = saved_symbols;

    // Evaluate the inner expression so it participates in checking. An
    // asserted function expression is contextually typed by the
    // assertion target (`((arg) => …) as Ctor["create"]`), so its
    // parameters are not implicit-any — resolve the target first and
    // pass it down.
    match asserted_expression.as_ref() {
        ParsedExpression::ArrowFunction(arrow) => {
            let contextual = match resolved_type.peeled() {
                surge_ts_types::Type::Function(function_type) => Some(function_type),
                _ => None,
            };
            // A target surge could not reduce to a signature still gives
            // tsc one, so an implicit-any report here would describe that
            // gap rather than the source.
            let degraded = contextual.is_none();
            if degraded {
                ctx.degraded_expected_type_depth += 1;
            }
            let _ = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                crate::checks::function::check_arrow_function_expression_with_expected_type(
                    arrow.as_ref().clone(),
                    contextual.as_ref(),
                    symbols,
                    ctx,
                )
            });
            if degraded {
                ctx.degraded_expected_type_depth -= 1;
            }
        }
        _ => {
            let source = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );
            // TS2352 between object types is withheld: the comparable relation
            // is only as good as surge's structural expansion of a library's
            // generic types, and on real code (zod's
            // `issue as errors.$ZodStringFormatIssues`) that produced 54 false
            // positives against a project that was otherwise diagnostic-exact.
            // Between primitives and their literals no expansion is involved.
            if let InferredExpression::Known(source_type) = &source
                && is_primitive_assertion_side(source_type)
                && is_primitive_assertion_side(&resolved_type)
            {
                super::assertion::check_assertion_overlap(&source, &resolved_type, fallback_span, ctx);
            }
        }
    }

    // If the type is unresolved (e.g. unknown named type), map_parsed_type
    // already emits TS2304 and returns Type::Unknown.
    // We just return it as the assertion result.
    InferredExpression::Known(resolved_type)
}

fn evaluate_non_null_assertion(
    asserted_expression: &Box<ParsedExpression>,
    expression_span: &Option<SyntaxTextSpan>,
    in_optional_chain: &bool,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let inferred = evaluate_expression(
        asserted_expression,
        expression_span.or(fallback_span),
        symbols,
        ctx,
    );

    match inferred {
        InferredExpression::Known(ty) => {
            let filtered = surge_ts_types::remove_nullish(&ty);
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

fn evaluate_property_access(
    object: &Box<ParsedExpression>,
    object_span: &Option<SyntaxTextSpan>,
    property_name: &String,
    property_span: &Option<SyntaxTextSpan>,
    is_bracketed: &bool,
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    // The receiver is code too: a call's bad argument, a missing member
    // deeper in the chain, a possibly-`undefined` link — inferring it
    // types the access but reports none of that.
    let receiver = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
    check_property_receiver(object, &receiver, *object_span, fallback_span, symbols, ctx);
    // `c["x"]` is tsc's deliberate escape hatch: element access skips the
    // accessibility check that `c.x` gets.
    if !*is_bracketed
        && let InferredExpression::Known(receiver_type) = &receiver
    {
        check_member_accessibility(
            object,
            receiver_type,
            property_name,
            *property_span,
            false,
            symbols,
            ctx,
        );
    }
    let inferred_expression = infer_expression(expression, symbols, ctx);
    if *is_bracketed
        && let InferredExpression::MissingProperty { object_type, .. } = &inferred_expression
    {
        report_missing_element(
            property_name,
            &Type::StringLiteral(property_name.clone()),
            object_type,
            *property_span,
            element_access_span(*object_span, *property_span).or(fallback_span),
            symbols,
            ctx,
        );
        return InferredExpression::Unknown;
    }
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
        | ParsedExpression::BigIntLiteral(_)
        | ParsedExpression::BooleanLiteral(_) => true,
        ParsedExpression::ArrayLiteral { elements, .. } => elements
            .iter()
            .all(|element| is_pure_literal_tree(&element.expression)),
        ParsedExpression::ObjectLiteral { properties, .. } => properties.iter().all(|property| {
            !property.is_spread && !property.is_method && is_pure_literal_tree(&property.value)
        }),
        _ => false,
    }
}

/// The contextual type a `??`/`||` default operand inherits from the operand it
/// falls back from. Only a function-typed left operand qualifies: that is the
/// case whose parameters would otherwise become implicit `any`, and widening it
/// further would start excess-property-checking object-literal defaults against
/// a type the author never wrote.
fn contextual_default_operand_type(left: &InferredExpression) -> Option<Type> {
    let InferredExpression::Known(left_type) = left else {
        return None;
    };
    let stripped = surge_ts_types::remove_nullish(left_type);
    matches!(stripped.peeled(), Type::Function(_) | Type::Object(_)).then_some(stripped)
}

/// The type tsc gives the `{}` in the `opts ?? {}` idiom. An object literal is
/// contextually typed by the binding pattern it initializes, so the empty
/// fallback carries the left operand's property *names* with type `undefined`;
/// reads off the joined union then answer `T | undefined` rather than reporting
/// the name as missing from a bare `{}`. `None` for any other right operand.
pub(crate) fn empty_object_fallback_type(
    right: &ParsedExpression,
    contextual: &Type,
) -> Option<Type> {
    let ParsedExpression::ObjectLiteral { properties, .. } = right else {
        return None;
    };
    if !properties.is_empty() {
        return None;
    }
    let Type::Object(contextual_object) = contextual.peeled() else {
        return None;
    };
    let mut absent = surge_ts_types::PropertyMap::default();
    for name in contextual_object.properties.keys() {
        absent.insert(
            name.clone(),
            surge_ts_types::ObjectProperty::optional(Type::Undefined),
        );
    }
    // tsc subtype-reduces `x ?? {}` to `x`'s type, so an index signature on it
    // survives: `(schema.properties ?? {})['data']` still reads the element.
    let string_index_type = contextual_object.string_index_type.as_deref().cloned();
    Some(Type::Object(surge_ts_types::ObjectType::new(
        absent,
        string_index_type,
    )))
}

/// For an equality operator, the result it has when its operands differ:
/// `false` for `==`/`===`, `true` for `!=`/`!==`.
fn equality_result_when_unequal(operator: surge_ts_syntax::ParsedBinaryOperator) -> Option<bool> {
    match operator {
        surge_ts_syntax::ParsedBinaryOperator::StrictEquals | surge_ts_syntax::ParsedBinaryOperator::Equals => Some(false),
        surge_ts_syntax::ParsedBinaryOperator::StrictNotEquals | surge_ts_syntax::ParsedBinaryOperator::NotEquals => Some(true),
        _ => None,
    }
}

/// tsc's equality checks that need only syntax: comparing against an object,
/// array or `function` literal can never be true, since objects compare by
/// reference (TS2839), and comparing against the global `NaN` never either
/// (TS2845).
fn report_reference_and_nan_equality(
    left: &ParsedExpression,
    right: &ParsedExpression,
    unequal_result: bool,
    span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let result = if unequal_result { "true" } else { "false" };
    let is_object_literal = |expression: &ParsedExpression| match expression {
        ParsedExpression::ObjectLiteral { .. } | ParsedExpression::ArrayLiteral { .. } => true,
        // A `function` expression lowers to the arrow shape but binds its own
        // `this`; tsc exempts a real arrow.
        ParsedExpression::ArrowFunction(arrow) => {
            !matches!(arrow.this_binding, surge_ts_syntax::ParsedThisBinding::Inherited)
        }
        _ => false,
    };
    if is_object_literal(left) || is_object_literal(right) {
        ctx.push(diagnostic_with_syntax_span(Diagnostic::ts2839(result, ctx.file_name.clone()), span));
    }
    // The lib's `declare var NaN` binds as a `let`-like global; a parameter or
    // block-scoped constant of the same name is not the global.
    let is_global_nan = |expression: &ParsedExpression| {
        matches!(expression, ParsedExpression::Identifier { name, .. } if name == "NaN")
            && symbols
                .get("NaN")
                .is_some_and(|symbol| {
                    matches!(
                        symbol.kind,
                        crate::symbols::SymbolKind::Var | crate::symbols::SymbolKind::Let
                    )
                })
    };
    if is_global_nan(left) || is_global_nan(right) {
        ctx.push(diagnostic_with_syntax_span(Diagnostic::ts2845(result, ctx.file_name.clone()), span));
    }
}

/// tsc's `checkInExpression`: the key must be assignable to
/// `string | number | symbol`, and the right operand must be non-nullable and
/// assignable to `object`. Operands surge could not settle are not judged.
fn check_in_operands(
    right: &ParsedExpression,
    left_result: &InferredExpression,
    right_result: &InferredExpression,
    left_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let judgeable = |ty: &Type| {
        !crate::checks::function::type_contains_unknown(ty)
            && !matches!(ty, Type::Any | Type::GenuineUnknown | Type::ErrorType)
    };
    let property_key = surge_ts_types::union_type(vec![Type::String, Type::Number, Type::Symbol]);
    let key_assignable = |ty: &Type| surge_ts_types::is_assignable_to(ty, &property_key);
    if let InferredExpression::Known(left_type) = left_result
        && judgeable(left_type)
        && !key_assignable(left_type)
    {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2322(
                &source_display_name(left_type, &Type::String),
                "string | number | symbol",
                ctx.file_name.clone(),
            ),
            left_span,
        ));
    }
    let InferredExpression::Known(right_type) = right_result else {
        return;
    };
    if !judgeable(right_type) {
        return;
    }
    let right_type = super::strip_reported_undefined_receiver(
        right,
        right_type.clone(),
        right_span,
        right_span,
        symbols,
        ctx,
    );
    fn is_primitive(ty: &Type) -> bool {
        match ty {
            Type::String
            | Type::Number
            | Type::Boolean
            | Type::BigInt
            | Type::Symbol
            | Type::StringLiteral(_)
            | Type::NumberLiteral(_)
            | Type::BooleanLiteral(_) => true,
            Type::Union(union) => union.types().iter().any(is_primitive),
            _ => false,
        }
    }
    if is_primitive(&right_type) {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2322(&right_type.name(), "object", ctx.file_name.clone()),
            right_span,
        ));
    }
}

/// tsc's `+`/`-`/`~` arm of `checkPrefixUnaryExpression`: a possibly nullish
/// operand is reported (`checkNonNullType`), a symbol operand is TS2469, and
/// unary `+` on a bigint is TS2736.
fn check_numeric_unary_operand(
    operator: ParsedUnaryOperator,
    operand: &ParsedExpression,
    operand_type: &Type,
    operand_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if crate::checks::function::type_contains_unknown(operand_type)
        || matches!(operand_type, Type::Any | Type::GenuineUnknown | Type::ErrorType)
    {
        return;
    }
    let operator_text = match operator {
        ParsedUnaryOperator::Plus => "+",
        ParsedUnaryOperator::Minus => "-",
        _ => "~",
    };
    super::maybe_emit_possibly_undefined_receiver(
        operand,
        operand_type,
        operand_span,
        operand_span,
        symbols,
        ctx,
    );
    fn some_member(ty: &Type, test: &dyn Fn(&Type) -> bool) -> bool {
        match ty {
            Type::Union(union) => union.types().iter().any(|member| some_member(member, test)),
            other => test(other),
        }
    }
    if some_member(operand_type, &|ty| matches!(ty, Type::Symbol)) {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2469(operator_text, ctx.file_name.clone()),
            operand_span,
        ));
    }
    if matches!(operator, ParsedUnaryOperator::Plus)
        && some_member(operand_type, &|ty| matches!(ty, Type::BigInt))
    {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2736("+", &widen_type(operand_type).name(), ctx.file_name.clone()),
            operand_span,
        ));
    }
}

fn type_may_be_symbol(ty: &Type) -> bool {
    match ty {
        Type::Symbol => true,
        Type::Union(union) => union.types().iter().any(type_may_be_symbol),
        _ => false,
    }
}

fn is_primitive_assertion_side(ty: &Type) -> bool {
    match ty {
        Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::Symbol
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(is_primitive_assertion_side),
        _ => false,
    }
}

/// The signature a tagged template calls when surge can relate its arguments
/// directly: a single, non-generic function with resolved parameters.
pub(crate) fn tagged_template_signature(tag: &Type) -> Option<surge_ts_types::FunctionType> {
    let peeled;
    let function = match tag {
        Type::Function(function) => function,
        Type::Reference(_) | Type::Object(_) => {
            peeled = tag.peeled();
            match &peeled {
                Type::Function(function) => function,
                Type::Object(object) => object.call_signature()?,
                _ => return None,
            }
        }
        _ => return None,
    };
    if function.overloads().is_some()
        || function
            .parameters()
            .iter()
            .chain([function.return_type()])
            .any(crate::checks::function::type_contains_degradation)
    {
        return None;
    }
    Some(function.clone())
}

fn check_tagged_template_argument(
    signature: &surge_ts_types::FunctionType,
    position: usize,
    argument: &Type,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    let parameters = signature.parameters();
    let parameter = if signature.is_variadic() && position + 1 >= parameters.len() {
        match parameters.last().map(Type::peeled) {
            Some(Type::Array(element)) => *element,
            _ => return false,
        }
    } else {
        match parameters.get(position) {
            Some(parameter) => parameter.clone(),
            None => return false,
        }
    };
    if argument.is_unknown() || surge_ts_types::is_assignable_to(argument, &parameter) {
        return false;
    }
    let parameter = super::reported_relation_target(argument, &parameter);
    let source_name = super::source_display_name(argument, &parameter);
    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts2345(&source_name, &parameter.name(), ctx.file_name.clone()),
        span,
    ));
    true
}
