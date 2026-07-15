use super::*;

pub(super) fn evaluate_optional_index_access(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_result = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);

    let object_type = match object_result {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = surge_ts_types::remove_undefined(&object_type);

    match base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = crate::infer::tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => {
                        InferredExpression::Known(union_type(vec![element_type, Type::Undefined]))
                    }
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                union_type(elements.to_vec()),
                Type::Undefined,
            ]))
        }
        Type::Array(element_type) => {
            if element_type.as_ref().is_unknown() {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    element_type.as_ref().clone()
                }),
                Type::Undefined,
            ]))
        }
        _ => InferredExpression::Unknown,
    }
}

pub(super) fn evaluate_index_access(
    object_name: &str,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name) else {
        if emit_type_only_as_value_diagnostic(object_name, object_span, ctx) {
            return InferredExpression::Unknown;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(object_name, ctx.file_name.clone()),
            choose_span(object_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    };

    match &symbol.ty {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => InferredExpression::Known(element_type),
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(elements.to_vec()))
        }
        Type::Array(element_type) => {
            if element_type.as_ref().is_unknown() {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(with_type_copy_reason(
                TypeCopyReason::ExpressionInference,
                || element_type.as_ref().clone(),
            ))
        }
        Type::Object(_)
        | Type::Function(_)
        | Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::Symbol
        | Type::Void
        | Type::Never
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Undefined
        | Type::Reference(_)
        | Type::Union(_) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                _ => return InferredExpression::Unknown,
            };

            // A literal index names a concrete property that must exist on the
            // receiver; a missing one is a real TS2339. A *non-literal* computed
            // key (plain `string`/`number`, `keyof T`, a type parameter, `symbol`,
            // …) resolves to an indexed-access type (`T[K]`, an index-signature
            // value, …) and is NOT a missing-property error — emitting one here was
            // a false positive that mis-named the receiver as the absent property.
            //
            // Only an object-like receiver can be missing a literal-named member.
            // Primitives carry an apparent type with index signatures (`string`'s
            // numeric index returns `string`, `string["length"]` is a real member,
            // …), so a literal index there is never a TS2339 — emitting one was a
            // false positive (`path[0]` reported as `Property 'path' ... 'string'`).
            let receiver_is_object_like = matches!(
                symbol.ty,
                Type::Object(_) | Type::Function(_) | Type::Reference(_)
            );
            if receiver_is_object_like
                && matches!(
                    index_type,
                    Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_)
                )
            {
                let object_type_name = symbol.ty.name();
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone()),
                    choose_span(object_span, fallback_span),
                ));
            }
            InferredExpression::Unknown
        }
    }
}

fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}
