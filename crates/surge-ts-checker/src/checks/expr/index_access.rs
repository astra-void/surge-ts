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
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => InferredExpression::Unknown,
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
            unresolved_name_diagnostic(object_name, symbols, ctx),
            choose_span(object_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    };
    if let Some(narrowed) = crate::infer::narrowed_element_read_named(object_name, index, symbols)
    {
        return InferredExpression::Known(narrowed);
    }

    let receiver_type = strip_reported_undefined_receiver(
        &ParsedExpression::Identifier {
            name: object_name.to_string(),
            span: object_span,
        },
        symbol.ty.clone(),
        object_span,
        fallback_span,
        symbols,
        ctx,
    );

    // A nominal array reference (`Array<number>`, `ReadonlyArray<string>`)
    // indexes like the array it names; left unpeeled it fell through to the
    // object arm and reported the *receiver* as a missing property.
    let receiver_type = match &receiver_type {
        // A reference to the library `Array<T>` interface itself (a `Array<string>`
        // return annotation reaching the caller through a package re-export)
        // peels to the interface's member object, which has no numeric index
        // signature to answer `key[0]`; it indexes like `T[]` all the same.
        Type::Reference(reference)
            if reference.arguments.len() == 1
                && matches!(
                    reference.id.split('\u{0}').next_back(),
                    Some("Array" | "ReadonlyArray")
                ) =>
        {
            Type::Array(Box::new(reference.arguments[0].clone()))
        }
        Type::Reference(_) => match receiver_type.peeled() {
            peeled @ (Type::Array(_) | Type::Tuple(_)) => peeled,
            Type::OpenTuple(tuple) => Type::Array(Box::new(tuple.element_union())),
            _ => receiver_type,
        },
        // An open tuple has no fixed length to index by, so a read off it is a
        // read off the array of everything it can hold.
        Type::OpenTuple(tuple) => Type::Array(Box::new(tuple.element_union())),
        _ => receiver_type,
    };

    match &receiver_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => InferredExpression::Unknown,
        // Lowered to its element array above.
        Type::OpenTuple(_) => InferredExpression::Unknown,
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

            InferredExpression::Known(crate::infer::unchecked_index_read(
                union_type(elements.to_vec()),
                ctx,
            ))
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

            InferredExpression::Known(crate::infer::unchecked_index_read(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    element_type.as_ref().clone()
                }),
                ctx,
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
                receiver_type,
                Type::Object(_) | Type::Function(_) | Type::Reference(_)
            );
            let Some(key) = literal_index_key(&index_type) else {
                return InferredExpression::Unknown;
            };

            // A nominal alias can peel to a tuple, or to a union distributing
            // over them (`type ResultTuple<T> = [undefined, T] | [Err, undefined]`).
            // The `Type::Tuple` arm above only sees an *unpeeled* tuple, so a
            // literal index on the alias fell through to here and was reported as
            // a property missing from the receiver — named after the receiver
            // binding, since that is what this arm has in hand.
            if let Some(element_type) = literal_tuple_element(&receiver_type.peeled(), &key) {
                return InferredExpression::Known(element_type);
            }

            // A literal key names a member. An index signature answers it too —
            // a numeric key is converted to a string, which is why
            // `record[1]` reads a `Record<number, T>` and a `Record<string, T>`
            // alike. Only a receiver that declares neither is a real TS2339.
            if let Type::Object(object_type) = receiver_type.peeled() {
                if let Some(member) = object_type.get_property_access_type(&key) {
                    return InferredExpression::Known(member);
                }
                if let Some(index_type) = object_type.string_index_type.as_deref() {
                    return InferredExpression::Known(crate::infer::unchecked_index_read(
                        index_type.clone(),
                        ctx,
                    ));
                }
            }

            if receiver_is_object_like {
                let object_type_name = receiver_type.name();
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone()),
                    choose_span(object_span, fallback_span),
                ));
            }
            InferredExpression::Unknown
        }
    }
}

/// The element a literal numeric key selects out of a tuple, distributing over a
/// union of them. `None` when the receiver is not tuple-shaped, or when any
/// member lacks the index — an out-of-range key is a real error, left to the
/// caller.
fn literal_tuple_element(ty: &Type, key: &str) -> Option<Type> {
    match ty {
        Type::Tuple(elements) => elements.get(key.parse::<usize>().ok()?).cloned(),
        Type::Union(union) => {
            let members = union.types();
            if members.is_empty() {
                return None;
            }
            let mut selected = Vec::with_capacity(members.len());
            for member in members {
                selected.push(literal_tuple_element(&member.peeled(), key)?);
            }
            Some(union_type(selected))
        }
        _ => None,
    }
}

/// The member name a literal computed key stands for. A string index signature
/// is keyed by the *string* form, so `1` and `"1"` name the same member.
fn literal_index_key(index_type: &Type) -> Option<String> {
    match index_type {
        Type::StringLiteral(value) => Some(value.clone()),
        Type::NumberLiteral(NumberLiteralType { value }) => Some(value.clone()),
        Type::BooleanLiteral(value) => Some(value.to_string()),
        _ => None,
    }
}

fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}
