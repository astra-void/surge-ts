use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::ParsedAssignment;
use surge_ts_types::is_assignable_to;

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::context::{CheckerContext, convert_span};
use crate::program::{record_assignability_check, record_program_timing};
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

    let Some(target) = symbols.get(&assignment.target_name) else {
        if emit_type_only_as_value_diagnostic(&assignment.target_name, Some(target_span), ctx) {
            return;
        }

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
            let assignability_start = Instant::now();
            record_assignability_check();
            if inferred_value_type != surge_ts_types::Type::Unknown
                && !type_contains_unknown(&target.ty)
                && !type_contains_unknown(&inferred_value_type)
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
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += assignability_start.elapsed()
            });
        }
        crate::infer::InferredExpression::UnresolvedIdentifier { .. } => {}
        crate::infer::InferredExpression::MissingProperty { .. } => {}
        crate::infer::InferredExpression::Unknown => {}
    }
}

fn type_contains_unknown(ty: &surge_ts_types::Type) -> bool {
    match ty {
        surge_ts_types::Type::Unknown => true,
        surge_ts_types::Type::Array(element) => type_contains_unknown(element),
        surge_ts_types::Type::Tuple(elements) => elements.iter().any(type_contains_unknown),
        surge_ts_types::Type::Function(function) => {
            function.parameters().iter().any(type_contains_unknown)
                || type_contains_unknown(function.return_type())
        }
        surge_ts_types::Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_contains_unknown(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(type_contains_unknown)
        }
        surge_ts_types::Type::Union(union) => union.types().iter().any(type_contains_unknown),
        _ => false,
    }
}
