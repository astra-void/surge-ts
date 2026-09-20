use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::ParsedAssignment;
use surge_ts_types::is_assignable_to;

use super::expected::{
    ExpectedTypeDiagnostic, evaluate_expression_with_expected_type_anchored,
};
use crate::context::{CheckerContext, convert_span};
use crate::program::{
    DtsExpansionReason, record_assignability_check, record_program_timing,
    with_dts_expansion_reason,
};
use crate::symbols::{SymbolKind, SymbolTable};

pub(crate) fn check_assignment(assignment: ParsedAssignment, ctx: &mut CheckerContext) {
    let symbols = ctx.symbols.clone();
    check_assignment_with_symbols(assignment, &symbols, false, ctx);
}

pub(crate) fn check_assignment_with_symbols(
    assignment: ParsedAssignment,
    symbols: &SymbolTable,
    shadowed_locally: bool,
    ctx: &mut CheckerContext,
) {
    let Some(target_span) = assignment.target_span else {
        return;
    };

    let Some(target) = symbols.get(&assignment.target_name) else {
        crate::checks::expr::report_unresolved_value_name(
            &assignment.target_name,
            Some(target_span),
            crate::checks::expr::UnresolvedNameSite::Reference,
            symbols,
            ctx,
        );
        return;
    };

    if !shadowed_locally && ctx.is_import_binding(&assignment.target_name) {
        let diagnostic = Diagnostic::ts2632(&assignment.target_name, ctx.file_name.clone())
            .with_span(convert_span(target_span));
        ctx.push(diagnostic);
        return;
    }

    if matches!(target.kind, SymbolKind::Const) {
        let diagnostic = Diagnostic::ts2588(&assignment.target_name, ctx.file_name.clone())
            .with_span(convert_span(target_span));
        ctx.push(diagnostic);
        return;
    }

    // An assignment is checked against the *declared* type, not the type flow
    // narrowing installed for the enclosing branch: inside
    // `if (v === undefined) { v = "x" }` the target is still `string | undefined`.
    let target_type = symbols
        .declared_type(&assignment.target_name)
        .unwrap_or(&target.ty)
        .clone();

    // tsc reports a mismatched write at the assignment target, elaborating into
    // the value only where it can (an object or array literal member).
    let inferred_value = evaluate_expression_with_expected_type_anchored(
        &assignment.value,
        assignment.value_span,
        Some(target_span),
        Some(&target_type),
        ExpectedTypeDiagnostic::TypeNotAssignable,
        symbols,
        ctx,
    );

    match inferred_value {
        crate::infer::InferredExpression::Known(inferred_value_type) => {
            let assignability_start = Instant::now();
            record_assignability_check();
            if inferred_value_type != surge_ts_types::Type::Unknown
                && !type_contains_unknown(&target_type)
                && !type_contains_unknown(&inferred_value_type)
                && !with_dts_expansion_reason(DtsExpansionReason::Assignability, || {
                    is_assignable_to(&inferred_value_type, &target_type)
                })
            {
                // tsc generalizes a literal source unless the target could hold
                // one (`reportRelationError`): `t = 1` into `string` reads `number`.
                let inferred_type_name =
                    crate::checks::expr::source_display_name(&inferred_value_type, &target_type);
                let target_type_name = target_type.name();
                let diagnostic = crate::checks::expr::type_not_assignable_diagnostic(
                    &inferred_value_type,
                    &target_type,
                    &inferred_type_name,
                    &target_type_name,
                    ctx.file_name.clone(),
                );

                let diagnostic = diagnostic.with_span(convert_span(target_span));
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

pub(crate) fn type_contains_unknown(ty: &surge_ts_types::Type) -> bool {
    super::structural_walk::query(|| contains_unknown(ty))
}

fn contains_unknown(ty: &surge_ts_types::Type) -> bool {
    match super::structural_walk::visit(ty) {
        super::structural_walk::Visit::Walk => {}
        super::structural_walk::Visit::Seen => return false,
        super::structural_walk::Visit::Exhausted => return true,
    }
    thread_local! {
        // References already on the walk, to break the cyclic structural graphs
        // lazy nominal references form (interface A whose member resolves to B
        // whose member resolves back to A). Re-entering one introduces no *new*
        // `unknown`, so it reports false — same guard as the return-type walker in
        // `checks::function::body`.
        static VISITING_REFERENCES: std::cell::RefCell<Vec<(std::sync::Arc<str>, std::sync::Arc<[surge_ts_types::Type]>)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    match ty {
        surge_ts_types::Type::Unknown | surge_ts_types::Type::TypeParameter(_) => true,
        // A degraded member hidden behind a lazy nominal reference must suppress
        // the comparison exactly as an inline one does: `is_assignable_to` peels
        // the reference and compares the unmodelled members structurally, so
        // without the peel here two degraded expansions of the same declaration
        // (`Server` reached through `ReturnType<typeof serve>` vs. through the
        // interface itself) mismatch and report a false TS2322.
        surge_ts_types::Type::Reference(reference) => {
            let on_path = VISITING_REFERENCES.with(|visiting| {
                visiting
                    .borrow()
                    .iter()
                    .any(|(id, arguments)| *id == reference.id && *arguments == reference.arguments)
                    // tsc's `isDeeplyNestedType`: a declaration on the path three
                    // times is an expanding recursion (`Deep<T>` reaching
                    // `Deep<T[]>`) whose arguments never repeat.
                    || visiting.borrow().iter().filter(|(id, _)| *id == reference.id).count() >= 3
            });
            if on_path {
                return false;
            }
            VISITING_REFERENCES.with(|visiting| {
                visiting
                    .borrow_mut()
                    .push((reference.id.clone(), reference.arguments.clone()));
            });
            let result = contains_unknown(&reference.resolve());
            VISITING_REFERENCES.with(|visiting| {
                visiting.borrow_mut().pop();
            });
            result
        }
        surge_ts_types::Type::Array(element) => contains_unknown(element),
        surge_ts_types::Type::Tuple(elements) => elements.iter().any(contains_unknown),
        surge_ts_types::Type::Function(function) => {
            function.parameters().iter().any(contains_unknown)
                || contains_unknown(function.return_type())
        }
        surge_ts_types::Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| contains_unknown(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(contains_unknown)
        }
        surge_ts_types::Type::Union(union) => union.types().iter().any(contains_unknown),
        _ => false,
    }
}
