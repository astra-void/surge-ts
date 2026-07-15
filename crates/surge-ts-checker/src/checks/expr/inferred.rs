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

            if let Some(suggestion) = suggested_unresolved_name(&name, ctx) {
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts2552(&name, suggestion, ctx.file_name.clone()),
                    choose_span(span, fallback_span),
                ));
                return;
            }

            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2304(&name, ctx.file_name.clone()),
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

pub(super) fn maybe_emit_unknown_property_receiver(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let InferredExpression::Known(Type::GenuineUnknown) = infer_expression(object, symbols, ctx)
    else {
        return;
    };

    let Some(name) = property_receiver_name(object) else {
        return;
    };

    ctx.push(diagnostic_with_syntax_span(
        Diagnostic::ts18046(name, ctx.file_name.clone()),
        choose_span(object_span, fallback_span),
    ));
}

fn property_receiver_name(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
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

fn suggested_unresolved_name(name: &str, ctx: &CheckerContext) -> Option<String> {
    let mut candidates = ctx
        .symbols
        .iter()
        .chain(ctx.ambient_global_symbols.iter())
        .map(|(candidate, _)| candidate.as_ref())
        .filter(|candidate| candidate.eq_ignore_ascii_case(name) && *candidate != name);

    candidates.next().map(|candidate| candidate.to_string())
}

fn is_missing_node_like_global(name: &str, ctx: &CheckerContext) -> bool {
    if ctx.options.types.iter().any(|ty| ty == "node") {
        return false;
    }

    matches!(name, "Buffer" | "process")
}
