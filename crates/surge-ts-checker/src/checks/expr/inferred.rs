use super::*;

pub(crate) fn report_inferred_expression(
    inferred_expression: InferredExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    match inferred_expression {
        InferredExpression::Known(known_type) => {
            if known_type.is_unknown() {
                return;
            }
        }
        InferredExpression::UnresolvedIdentifier { name, span } => {
            if emit_type_only_as_value_diagnostic(&name, span, ctx) {
                return;
            }

            // The name is unresolved *here* but a UMD global resolves it for
            // tsc, so the reference reports as TS2686 rather than as a missing
            // name.
            if crate::checks::emit_value_position_reference_diagnostic(
                &name,
                choose_span(span, fallback_span),
                ctx,
            ) {
                return;
            }

            if is_missing_node_like_global(&name, ctx) {
                let diagnostic = if ctx.options.types_uses_wildcard() {
                    Diagnostic::ts2580(&name, ctx.file_name.clone())
                } else {
                    Diagnostic::ts2591(&name, ctx.file_name.clone())
                };
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(span, fallback_span),
                ));
                return;
            }

            ctx.push(diagnostic_with_syntax_span(
                unresolved_name_diagnostic(&name, symbols, ctx),
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::MissingProperty {
            property_name,
            object_type,
            span,
        } => {
            let diagnostic = missing_property_diagnostic(
                &property_name,
                &object_type,
                symbols,
                ctx.file_name.clone(),
            );
            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::Unknown => {}
    }
}

/// The receiver of a non-optional member access, checked the way tsc's
/// `checkNonNullExpression` does: `unknown` is TS18046 named and TS2571
/// otherwise, and a receiver that can be `undefined` is TS18048 named and
/// TS2532 otherwise. The access itself proceeds on the non-`undefined` part, so
/// nothing cascades.
pub(super) fn check_property_receiver(
    object: &ParsedExpression,
    receiver: &InferredExpression,
    object_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let InferredExpression::Known(object_type) = receiver else {
        return;
    };
    if *object_type == Type::GenuineUnknown {
        let diagnostic = match nameable_receiver(object) {
            Some(name) => Diagnostic::ts18046(name, ctx.file_name.clone()),
            None => Diagnostic::ts2571(ctx.file_name.clone()),
        };
        ctx.push(diagnostic_with_syntax_span(
            diagnostic,
            choose_span(object_span, fallback_span),
        ));
        return;
    }
    maybe_emit_possibly_undefined_receiver(
        object,
        object_type,
        object_span,
        fallback_span,
        symbols,
        ctx,
    );
}

/// Reports a possibly-`undefined` receiver and returns the type the access
/// continues on.
pub(crate) fn strip_reported_undefined_receiver(
    object: &ParsedExpression,
    object_type: Type,
    object_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    if !maybe_emit_possibly_undefined_receiver(
        object,
        &object_type,
        object_span,
        fallback_span,
        symbols,
        ctx,
    ) {
        return object_type;
    }
    surge_ts_types::remove_undefined(&object_type)
}

/// Reports a possibly-`undefined` operand, the way tsc's `checkNonNullType`
/// does, and says whether it reported. Shared with the `++`/`--` operand rule.
pub(crate) fn maybe_emit_possibly_undefined_receiver(
    object: &ParsedExpression,
    object_type: &Type,
    object_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> bool {
    // A later link of an optional chain carries the chain's short-circuit
    // `undefined`, which is not the receiver's own: `b?.value.x` is fine, while
    // `b?.inner.x` with an optional `inner` is still an error. tsc keeps the
    // two apart with a marker; surge re-derives the link without it.
    // A `null` keyword is the same TS18050 as `undefined`, but surge types it
    // as `Any` (infer/expression/mod.rs) rather than a null type, so the
    // nullability gate below would never see it.
    if matches!(object, ParsedExpression::NullLiteral) {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts18050("null", ctx.file_name.clone()),
            choose_span(object_span, fallback_span),
        ));
        return true;
    }
    let receiver_type = if object.continues_optional_chain() {
        let Some(without_marker) = chain_link_type_without_marker(object, symbols, ctx) else {
            return false;
        };
        without_marker
    } else {
        object_type.clone()
    };
    if !receiver_can_be_undefined(&receiver_type) {
        return false;
    }
    // `undefined` names a value rather than a possibly-`undefined` place, so
    // tsc answers it with TS18050 before the possibly-`undefined` wording.
    let diagnostic = if matches!(object, ParsedExpression::UndefinedLiteral) {
        Diagnostic::ts18050("undefined", ctx.file_name.clone())
    } else {
        match nameable_receiver(object) {
            Some(name) => Diagnostic::ts18048(name, ctx.file_name.clone()),
            None => Diagnostic::ts2532(ctx.file_name.clone()),
        }
    };
    ctx.push(diagnostic_with_syntax_span(
        diagnostic,
        choose_span(object_span, fallback_span),
    ));
    true
}

/// The type of an optional-chain link with the chain's own `undefined` left
/// out: the last property read applied to the link's receiver with `undefined`
/// removed. `None` for a link surge cannot re-derive (a call, an element
/// access), which then reports nothing.
fn chain_link_type_without_marker(
    link: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match link {
        ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => {
            let InferredExpression::Known(base) = infer_expression(object, symbols, ctx) else {
                return None;
            };
            property_type_on(&surge_ts_types::remove_undefined(&base), property_name)
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => {
            let base = chain_link_type_without_marker(object, symbols, ctx)?;
            property_type_on(&surge_ts_types::remove_undefined(&base), property_name)
        }
        ParsedExpression::NonNullAssertion { expression, .. } => {
            chain_link_type_without_marker(expression, symbols, ctx)
                .map(|ty| surge_ts_types::remove_undefined(&ty))
        }
        _ => None,
    }
}

/// `base.name` read on a type, distributing over a union; `None` when any
/// member lacks the property or the base is unmodelled.
fn property_type_on(base: &Type, name: &str) -> Option<Type> {
    match base.peeled() {
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => None,
        Type::Union(union) => {
            let mut members = Vec::with_capacity(union.types().len());
            for member in union.types() {
                if member.is_unknown() {
                    return None;
                }
                if *member == Type::Undefined {
                    continue;
                }
                members.push(member.get_property_access_type(name)?);
            }
            Some(surge_ts_types::union_type(members))
        }
        other => other.get_property_access_type(name),
    }
}

/// A receiver tsc rejects as possibly `undefined`. A union with a degradation
/// sentinel is left alone: part of it is unmodelled, so the `undefined` member
/// may be surge's, not the source's. `void` is not nullable to tsc — a member
/// read on it is a missing property, reported elsewhere.
pub(crate) fn receiver_can_be_undefined(ty: &Type) -> bool {
    match ty.peeled() {
        Type::Undefined => true,
        Type::Union(union) => {
            let members = union.types();
            !members.iter().any(Type::is_unknown)
                && members.iter().any(|member| *member == Type::Undefined)
        }
        _ => false,
    }
}

/// The receiver name tsc is willing to render. `reportObjectPossiblyNullOrUndefinedError`
/// and its `unknown` sibling both drop to the unnamed diagnostic past 100 bytes
/// (`len(nodeText) < 100`), so a long dotted chain reports as TS2532/TS2571.
fn nameable_receiver(expression: &ParsedExpression) -> Option<String> {
    property_receiver_name(expression).filter(|name| name.len() < 100)
}

/// The entity name tsc renders for a receiver: an identifier or a dotted chain
/// of them (`b?.inner` renders as `b.inner`). A bracketed access is an element
/// access to tsc, so it has none.
fn property_receiver_name(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            is_bracketed: false,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            is_bracketed: false,
            ..
        } => {
            let mut name = property_receiver_name(object)?;
            name.push('.');
            name.push_str(property_name);
            Some(name)
        }
        _ => None,
    }
}

/// TS2304 for an unresolved value name, or TS2552 when a close in-scope name
/// suggests a typo.
pub(crate) fn unresolved_name_diagnostic(
    name: &str,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Diagnostic {
    match suggested_unresolved_name(name, symbols, ctx) {
        Some(suggestion) => Diagnostic::ts2552(name, suggestion, ctx.file_name.clone()),
        // A shorthand property names the value it reads, so tsc's message says
        // what to do about it instead of reporting a bare missing name. A
        // spelling suggestion still wins, as it does for any other reference.
        None if ctx.shorthand_property_depth > 0 => {
            Diagnostic::ts18004(name, ctx.file_name.clone())
        }
        None => Diagnostic::ts2304(name, ctx.file_name.clone()),
    }
}

/// tsc's `getSpellingSuggestion`: the closest in-scope value name within an
/// edit distance of `floor(len * 0.4)`, skipping candidates whose length
/// differs by more than `max(2, floor(len * 0.34))`. A case-only difference
/// costs a tenth of an edit, so it beats any real edit.
fn suggested_unresolved_name(
    name: &str,
    symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> Option<String> {
    let max_length_difference = 2.max(name.len() * 34 / 100);
    let mut best_distance = (name.len() * 4 / 10) as f64 + 1.0;
    let mut best: Option<&str> = None;
    let mut candidates: Vec<&str> = symbols
        .iter()
        .chain(ctx.symbols.iter())
        .chain(ctx.ambient_global_symbols.iter())
        .map(|(candidate, _)| candidate.as_ref())
        .collect();
    // Symbol tables have no declaration order to offer; a name order at least
    // keeps the tie-break deterministic.
    candidates.sort_unstable();
    candidates.dedup();
    for candidate in candidates {
        if candidate == name || candidate.len().abs_diff(name.len()) > max_length_difference {
            continue;
        }
        if candidate.len() < 3 && !candidate.eq_ignore_ascii_case(name) {
            continue;
        }
        let Some(distance) = levenshtein_with_max(name, candidate, best_distance - 0.1) else {
            continue;
        };
        if distance < best_distance {
            best_distance = distance;
            best = Some(candidate);
        }
    }
    best.map(str::to_string)
}

/// tsc's `getSpellingSuggestion` over an ordered candidate list. On a distance
/// tie the earlier candidate wins, which is tsc's order for union members.
/// More than `max_candidates` candidates (when non-zero) means no suggestion.
pub(crate) fn spelling_suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    max_candidates: usize,
) -> Option<&'a str> {
    let name_length = name.chars().count();
    let max_length_difference = 2.max(name_length * 34 / 100);
    let mut best_distance = (name_length * 4 / 10) as f64 + 0.9;
    let mut best: Option<&str> = None;
    for (index, candidate) in candidates.into_iter().enumerate() {
        if max_candidates > 0 && index >= max_candidates {
            return None;
        }
        if candidate.is_empty()
            || candidate == name
            || candidate.len().abs_diff(name_length) > max_length_difference
        {
            continue;
        }
        if candidate.len() < 3 && !candidate.eq_ignore_ascii_case(name) {
            continue;
        }
        let Some(distance) = levenshtein_with_max(name, candidate, best_distance) else {
            continue;
        };
        if distance < best_distance {
            best_distance = distance;
            best = Some(candidate);
        } else if best.is_none() {
            best = Some(candidate);
        }
    }
    best
}

/// Levenshtein distance where a case-only substitution costs 0.1, abandoning a
/// row once every cell exceeds `max`.
fn levenshtein_with_max(source: &str, target: &str, max: f64) -> Option<f64> {
    let source: Vec<char> = source.chars().collect();
    let target: Vec<char> = target.chars().collect();
    let mut previous: Vec<f64> = (0..=target.len()).map(|column| column as f64).collect();
    let mut current = vec![0.0; target.len() + 1];
    let big = max + 0.01;
    for (row, source_char) in source.iter().enumerate() {
        let row_number = row + 1;
        current[0] = row_number as f64;
        let min_column = if (row_number as f64) > max {
            row_number - max as usize - 1
        } else {
            1
        };
        let max_column = if (target.len() as f64) > max + row_number as f64 {
            row_number + max as usize
        } else {
            target.len()
        };
        let mut column_min = big;
        for column in 1..min_column {
            current[column] = big;
        }
        for column in min_column.max(1)..=max_column {
            let target_char = target[column - 1];
            let substitution = if *source_char == target_char {
                previous[column - 1]
            } else if source_char.to_lowercase().eq(target_char.to_lowercase()) {
                previous[column - 1] + 0.1
            } else {
                previous[column - 1] + 2.0
            };
            let deletion = previous[column] + 1.0;
            let insertion = current[column - 1] + 1.0;
            let value = substitution.min(deletion).min(insertion);
            current[column] = value;
            column_min = column_min.min(value);
        }
        for column in max_column + 1..=target.len() {
            current[column] = big;
        }
        if column_min > max {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    let result = previous[target.len()];
    (result <= max).then_some(result)
}

fn is_missing_node_like_global(name: &str, ctx: &CheckerContext) -> bool {
    if ctx.options.types.iter().any(|ty| ty == "node") {
        return false;
    }

    matches!(name, "Buffer" | "process")
}
