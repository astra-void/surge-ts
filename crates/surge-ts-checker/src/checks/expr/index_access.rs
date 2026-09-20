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

    let base_type = surge_ts_types::remove_nullish(&object_type);

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

            if !numeric_index_key(&index_type) && !indexes_as_number(index, symbols) {
                report_non_numeric_index(
                    &index_type,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                    ctx,
                );
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

            if !numeric_index_key(&index_type) && !indexes_as_number(index, symbols) {
                report_non_numeric_index(
                    &index_type,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                    ctx,
                );
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
        if object_name == "super" || emit_type_only_as_value_diagnostic(object_name, object_span, ctx) {
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

    // A user-defined thenable indexes like the value it resolves to: `await` is
    // erased at parse time, so `const rows = await db.execute(...)` reaches here
    // as the query object itself, exactly as a `Promise<T>` reaches it as `T`.
    let receiver_type = crate::checks::call::thenable_awaited_type(&receiver_type)
        .unwrap_or(receiver_type);

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
        _ => receiver_type,
    };

    if let Type::OpenTuple(tuple) = &receiver_type {
        return crate::infer::expression::infer_open_tuple_index_access(
            tuple,
            index,
            &index_span,
            symbols,
            ctx,
        );
    }

    match &receiver_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown
        | Type::GenuineUnknown
        | Type::ErrorType
        | Type::TypeParameter(_) => InferredExpression::Unknown,
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

            // tsc's `getPropertyTypeForIndexType`: a literal index past a fixed
            // tuple's end is TS2493 (a negative one TS2514) and reads `undefined`,
            // unless the access allows a missing element — a destructured
            // binding with a default.
            if let Some(index_value) = tuple_literal_index_value(&index_type) {
                if let Some(element_type) =
                    usize::try_from(index_value).ok().and_then(|index| elements.get(index))
                {
                    return InferredExpression::Known(element_type.clone());
                }
                report_tuple_index_out_of_bounds(
                    elements,
                    index_value,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                    ctx,
                );
                return InferredExpression::Known(Type::Undefined);
            }

            if !numeric_index_key(&index_type) && !indexes_as_number(index, symbols) {
                report_non_numeric_index(
                    &index_type,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                    ctx,
                );
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

            if !numeric_index_key(&index_type) && !indexes_as_number(index, symbols) {
                report_non_numeric_index(
                    &index_type,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                    ctx,
                );
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
        | Type::Null
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
            // A key that is not a literal still resolves through an index
            // signature: `record[k]` with `k: string` reads the string index,
            // and a numeric key prefers the number one
            // (`findApplicableIndexInfo`). A number-only receiver cannot answer
            // a string key at all — under `noImplicitAny` that is TS7015, tsc's
            // "index expression is not of type 'number'".
            let index_is_numeric = is_assignable_to(&index_type, &Type::Number)
                || indexes_as_number(index, symbols);
            let Some(key) = literal_index_key(&index_type) else {
                if let Type::Object(object_type) = receiver_type.peeled() {
                    if let Some(index_value) = object_type.applicable_index_type(index_is_numeric) {
                        return InferredExpression::Known(crate::infer::unchecked_index_read(
                            index_value.clone(),
                            ctx,
                        ));
                    }
                    if let Some(member_union) = union_literal_index_read(&object_type, &index_type)
                    {
                        return InferredExpression::Known(member_union);
                    }
                    if object_type.number_index_type.is_some() && ctx.options.no_implicit_any {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts7015(ctx.file_name.clone()),
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        return InferredExpression::Unknown;
                    }
                    // `getPropertyTypeForIndexType`: a key the receiver answers
                    // neither as a member nor through an index signature makes
                    // the whole access an implicit `any`, exactly as a literal
                    // key that misses does.
                    //
                    // Held to a member set surge wrote down itself: a nominal
                    // reference's index signature can come from a base, a
                    // mapped type or a `Record` surge has not expanded here
                    // (zod's `$ZodShape`, `RegExpExecArray`), and an empty
                    // object type is a modelling gap rather than a receiver
                    // that answers nothing.
                    if ctx.options.no_implicit_any
                        && index_key_is_concrete(&index_type)
                        && matches!(&receiver_type, Type::Object(written) if !written.properties.is_empty())
                    {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts7053(
                                index_type.name(),
                                receiver_type.name(),
                                ctx.file_name.clone(),
                            ),
                            element_access_span(object_span, index_span).or(fallback_span),
                        ));
                    }
                }
                // A `string` answers a numeric index and its own members and
                // nothing else, so any other key is the implicit `any` an
                // array's non-numeric index already is.
                if matches!(receiver_type, Type::String | Type::StringLiteral(_))
                    && !index_is_numeric
                    && index_key_is_concrete(&index_type)
                {
                    report_non_numeric_index(
                        &index_type,
                        choose_span(index_span, choose_span(object_span, fallback_span)),
                        ctx,
                    );
                }
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
                if let Some(index_value) = object_type.applicable_index_type(index_is_numeric) {
                    return InferredExpression::Known(crate::infer::unchecked_index_read(
                        index_value.clone(),
                        ctx,
                    ));
                }
                // A literal key a number-only receiver cannot answer is the same
                // implicit `any` as a non-literal one.
                if object_type.number_index_type.is_some() && ctx.options.no_implicit_any {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts7015(ctx.file_name.clone()),
                        choose_span(index_span, choose_span(object_span, fallback_span)),
                    ));
                    return InferredExpression::Unknown;
                }
            }

            if receiver_is_object_like {
                report_missing_element(
                    &key,
                    &index_type,
                    &receiver_type,
                    index_span,
                    element_access_span(object_span, index_span).or(fallback_span),
                    symbols,
                    ctx,
                );
            }
            InferredExpression::Unknown
        }
    }
}

/// tsc's `isForInVariableForNumericPropertyNames`: the key of a `for…in` over
/// an object whose only index signature is numeric indexes as a `number`, even
/// though the binding itself is a `string`.
/// A key an array-like receiver indexes by. tsc converts a numeric-like string
/// literal to its number, so `xs["0"]` reads element 0 and is not an error.
/// The written index of a literal tuple key, including the numeric-like string
/// literal tsc converts to a number. Negative values stay signed: an out-of-range
/// index is reported, not silently read as a member.
fn tuple_literal_index_value(index_type: &Type) -> Option<i64> {
    match index_type {
        Type::NumberLiteral(NumberLiteralType { value }) | Type::StringLiteral(value) => {
            value.parse::<i64>().ok()
        }
        _ => None,
    }
}

fn numeric_index_key(index_type: &Type) -> bool {
    match index_type {
        Type::StringLiteral(value) => value.parse::<f64>().is_ok(),
        other => is_assignable_to(other, &Type::Number),
    }
}

/// A receiver whose only index is numeric cannot answer any other key. tsc
/// reports a key that could still name a property as the implicit `any` the
/// access reads (TS7015, silent without `noImplicitAny`), and one that cannot
/// name a property at all as TS2538.
fn report_non_numeric_index(
    index_type: &Type,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    if matches!(index_type, Type::Boolean | Type::BooleanLiteral(_)) {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2538(index_type.name(), ctx.file_name.clone()),
            span,
        ));
        return;
    }
    if !ctx.options.no_implicit_any {
        return;
    }
    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts7015(ctx.file_name.clone()),
        span,
    ));
}

/// What `receiver[index]` reads off an object receiver, for the
/// diagnostic-free inference path: the member a literal key names, or the
/// applicable index signature. `None` wherever the checked path would report.
pub(crate) fn object_element_read(
    receiver_type: &Type,
    index: &ParsedExpression,
    index_type: &Type,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Option<Type> {
    let peeled = receiver_type.peeled();
    let index_is_numeric =
        is_assignable_to(index_type, &Type::Number) || indexes_as_number(index, symbols);
    let Some(key) = literal_index_key(index_type) else {
        let Type::Object(object_type) = &peeled else {
            return None;
        };
        return match object_type.applicable_index_type(index_is_numeric) {
            Some(index_value) => Some(crate::infer::unchecked_index_read(index_value.clone(), ctx)),
            None => union_literal_index_read(object_type, index_type),
        };
    };
    if let Some(element_type) = literal_tuple_element(&peeled, &key) {
        return Some(element_type);
    }
    let Type::Object(object_type) = &peeled else {
        return None;
    };
    object_type.get_property_access_type(&key).or_else(|| {
        object_type
            .applicable_index_type(index_is_numeric)
            .map(|index_value| crate::infer::unchecked_index_read(index_value.clone(), ctx))
    })
}

fn indexes_as_number(index: &ParsedExpression, symbols: &SymbolTable) -> bool {
    let ParsedExpression::Identifier { name, .. } = index else {
        return false;
    };
    symbols
        .get(name)
        .is_some_and(|symbol| matches!(symbol.kind, crate::symbols::SymbolKind::ForInNumericKey))
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
/// A union of literal keys indexes like each of them: `o[k]` with `k: "a" | "b"`
/// reads `o["a"] | o["b"]`, and is only a missing key when one of them is. The
/// union never reaches [`literal_index_key`], so without this it would look like
/// a non-literal key the receiver cannot answer.
fn union_literal_index_read(
    object_type: &surge_ts_types::ObjectType,
    index_type: &Type,
) -> Option<Type> {
    let Type::Union(union) = index_type else {
        return None;
    };
    let members = union.types();
    if members.is_empty() {
        return None;
    }
    let mut member_types = Vec::with_capacity(members.len());
    for member in members {
        let key = literal_index_key(member)?;
        member_types.push(object_type.get_property_access_type(&key)?);
    }
    Some(union_type(member_types))
}

/// Whether an index type is specific enough to hold the receiver to it. A
/// degraded or still-open key (`any` because surge could not model the
/// expression, an unresolved reference, an uninstantiated type parameter)
/// answers nothing about which member is read.
fn index_key_is_concrete(index_type: &Type) -> bool {
    !matches!(
        index_type,
        Type::Any
            | Type::Unknown
            | Type::GenuineUnknown
            | Type::ErrorType
            | Type::TypeParameter(_)
            | Type::Reference(_)
    )
}

fn literal_index_key(index_type: &Type) -> Option<String> {
    match index_type {
        Type::StringLiteral(value) => Some(value.clone()),
        Type::NumberLiteral(NumberLiteralType { value }) => Some(value.clone()),
        Type::BooleanLiteral(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn report_tuple_index_out_of_bounds(
    elements: &[Type],
    index_value: i64,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    if ctx.allow_missing_tuple_element {
        return;
    }
    let diagnostic = if index_value < 0 {
        Diagnostic::ts2514(ctx.file_name.clone())
    } else {
        Diagnostic::ts2493(
            Type::Tuple(elements.to_vec()).name(),
            elements.len(),
            index_value,
            ctx.file_name.clone(),
        )
    };
    ctx.push(diagnostic_with_syntax_span(diagnostic, span));
}
