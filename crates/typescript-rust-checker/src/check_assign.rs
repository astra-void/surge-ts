use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::ParsedAssignment;
use typescript_rust_types::is_assignable_to;

use crate::check_expr::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::context::{CheckerContext, convert_span};
use crate::symbols::{SymbolKind, SymbolTable};

pub(crate) fn check_assignment(assignment: ParsedAssignment, ctx: &mut CheckerContext) {
    let symbols = ctx.symbols.clone();
    check_assignment_with_symbols(assignment, &symbols, ctx);
}

pub(crate) fn check_assignment_with_symbols(
    assignment: ParsedAssignment,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(target_span) = assignment.target_span else {
        return;
    };

    let Some(target) = symbols.get(&assignment.target_name).cloned() else {
        let diagnostic = Diagnostic::ts2304(&assignment.target_name, ctx.file_name.clone())
            .with_span(convert_span(target_span));
        ctx.push(diagnostic);
        return;
    };

    if matches!(target.kind, SymbolKind::Const) {
        let diagnostic = Diagnostic::ts2588(&assignment.target_name, ctx.file_name.clone())
            .with_span(convert_span(target_span));
        ctx.push(diagnostic);
        return;
    }

    let inferred_value = evaluate_expression_with_expected_type(
        &assignment.value,
        assignment.value_span,
        Some(&target.ty),
        ExpectedTypeDiagnostic::TypeNotAssignable,
        symbols,
        ctx,
    );

    match inferred_value {
        crate::infer::InferredExpression::Known(inferred_value_type) => {
            if inferred_value_type != typescript_rust_types::Type::Unknown
                && !is_assignable_to(&inferred_value_type, &target.ty)
            {
                let inferred_type_name = inferred_value_type.name();
                let target_type_name = target.ty.name();
                let diagnostic = Diagnostic::ts2322(
                    &inferred_type_name,
                    &target_type_name,
                    ctx.file_name.clone(),
                );

                let diagnostic = match assignment.value_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };

                ctx.push(diagnostic);
            }
        }
        crate::infer::InferredExpression::UnresolvedIdentifier { .. } => {}
        crate::infer::InferredExpression::MissingProperty { .. } => {}
        crate::infer::InferredExpression::Unknown => {}
    }
}
