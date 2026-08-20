pub(crate) mod assign;
pub(crate) mod call;
pub(crate) mod expected;
pub(crate) mod expr;
pub(crate) mod function;
pub(crate) mod jsx;
pub(crate) mod ops;
pub(crate) mod var;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, TextSpan};

use crate::context::{CheckerContext, convert_span};

/// Reports TS2686 when `name` reaches the file under check only as a UMD global.
/// Returns whether it fired, so an unresolved-name path can stop before its own
/// report: tsc resolves the name successfully here and never reaches its
/// cannot-find-name branch.
pub(crate) fn emit_umd_global_reference_diagnostic(
    name: &str,
    span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    if !ctx.is_umd_global_value_reference(name) {
        return false;
    }

    let mut diagnostic = Diagnostic::ts2686(name, ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
    true
}

/// The identifier a value expression ultimately reads, peeled through member
/// access and calls: `React.createElement(x)` reads `React`. tsc reports a UMD
/// reference at that head identifier, not at the whole expression.
fn value_reference_head(expression: &ParsedExpression) -> Option<(&str, Option<TextSpan>)> {
    match expression {
        ParsedExpression::Identifier { name, span } => Some((name.as_str(), *span)),
        ParsedExpression::Call {
            callee_name,
            callee_span,
            ..
        } => Some((callee_name.as_str(), *callee_span)),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            ..
        } => Some((object_name.as_str(), *object_span)),
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. }
        | ParsedExpression::PropertyCall { object, .. }
        | ParsedExpression::OptionalPropertyCall { object, .. }
        | ParsedExpression::ElementAccess { object, .. }
        | ParsedExpression::OptionalIndexAccess { object, .. } => value_reference_head(object),
        ParsedExpression::New { callee, .. } | ParsedExpression::OptionalCall { callee, .. } => {
            value_reference_head(callee)
        }
        ParsedExpression::NonNullAssertion { expression, .. } => value_reference_head(expression),
        _ => None,
    }
}

/// Reports a UMD-global read at the head of a value expression. Cheap to call on
/// every evaluated expression: the per-file name set is empty in every program
/// that has no UMD global, and duplicate spans collapse in `ctx.push`.
pub(crate) fn check_umd_global_value_reference(
    expression: &ParsedExpression,
    fallback_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    if ctx.file_umd_global_names.is_empty() {
        return;
    }

    let Some((name, span)) = value_reference_head(expression) else {
        return;
    };

    if !ctx.is_umd_global_value_reference(name) {
        return;
    }

    let name = name.to_string();
    emit_umd_global_reference_diagnostic(&name, span.or(fallback_span), ctx);
}

pub(crate) fn emit_type_only_as_value_diagnostic(
    name: &str,
    span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    if ctx.lookup_type_declaration(name).is_none() {
        return false;
    }

    let mut diagnostic = Diagnostic::ts2693(name, ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
    true
}
