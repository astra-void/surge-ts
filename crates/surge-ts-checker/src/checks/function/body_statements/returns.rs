
use surge_ts_syntax::{
    ParsedExpression, ParsedReturnStatement,
};
use surge_ts_types::{Type, is_assignable_to};

use crate::checks::expected::evaluate_return_expression_with_expected_type;
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    FlowCheck, FunctionFlowState, check_expression_flow,
};
use crate::infer::InferredExpression;
use crate::symbols::SymbolTable;

/// Whether a return value could possibly infer as `any`, so the probe below is
/// worth running. A literal, an object/array literal, an arrow or a JSX element
/// has a shape of its own and never is.
///
/// This is not only an optimization. The inference pass is diagnostic-free for
/// every kind EXCEPT an object literal: `infer_object_property_type` routes
/// method and accessor shorthand through the *checking* entry — deliberately, to
/// honor its declared signature — with no expected type, so probing an object
/// literal reported its methods' parameters as implicit any while the real
/// check, one pass later, typed them correctly from the contextual signature.
pub(super) fn may_infer_as_any(expression: &ParsedExpression) -> bool {
    !matches!(
        expression,
        ParsedExpression::ObjectLiteral { .. }
            | ParsedExpression::ArrayLiteral { .. }
            | ParsedExpression::ArrowFunction(_)
            | ParsedExpression::JsxElement { .. }
            | ParsedExpression::JsxFragment { .. }
            | ParsedExpression::StringLiteral(_)
            | ParsedExpression::NumberLiteral(_)
            | ParsedExpression::BigIntLiteral(_)
            | ParsedExpression::BooleanLiteral(_)
            | ParsedExpression::TemplateLiteral { .. }
            | ParsedExpression::UndefinedLiteral
            | ParsedExpression::NullLiteral
    )
}

/// Whether a return value's own type is `any`, directly or as a union member —
/// the shape that collapses tsc's inferred return type. See
/// `ContextualReturnFrame`.
pub(super) fn returns_any(inferred: &InferredExpression) -> bool {
    let InferredExpression::Known(ty) = inferred else {
        return false;
    };
    match ty {
        Type::Any => true,
        Type::Union(union) => union.types().iter().any(|member| *member == Type::Any),
        _ => false,
    }
}

pub(crate) fn check_function_return_statement(
    return_statement: ParsedReturnStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    flow_state: &mut FunctionFlowState,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(expression) = return_statement.expression.as_ref() else {
        return;
    };

    let flow_blocked = if flow_state.tracked_local_count() > 0 {
        check_expression_flow(
            expression,
            return_statement.expression_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if flow_blocked.is_blocked() {
        return;
    }

    let Some(return_type) = return_type else {
        // No expected return type only removes the assignability verdict; the
        // value still owes its own diagnostics. Skipping it left every
        // expression in an unannotated function unchecked — unresolved names,
        // UMD globals behind JSX tags, property access — which is why a
        // component written as `const C = () => { return <div/>; }` reported
        // nothing at all.
        // …but with no expectation there is also no contextual parameter type
        // to hand a callback or an object-literal method, so an implicit-any
        // report here would describe surge's missing context rather than an
        // omission in the source (tRPC's `new ReadableStream({ start(c) {…} })`
        // inside a returned object is typed by the constructor, not by the
        // return). Same reasoning as a degraded expectation.
        ctx.degraded_expected_type_depth += 1;
        let inferred = evaluate_expression(
            expression,
            return_statement.expression_span,
            symbols,
            ctx,
        );
        ctx.degraded_expected_type_depth -= 1;
        match inferred {
            InferredExpression::Known(source_type) => ctx.note_contextual_return_type(&source_type),
            // A value surge could not type still counts as returned: tsc knows
            // its type and decides TS7030 from it, so the sentinel must suppress
            // the report the same way a known `unknown` result does.
            _ => ctx.note_contextual_return_type(&Type::Unknown),
        }
        return;
    };

    // Only the mismatch verdicts raised while checking this value belong to the
    // contextual-return frame; everything else the expression reports is
    // unrelated and must survive.
    let was_in_return_check = ctx.in_contextual_return_check;
    ctx.in_contextual_return_check = ctx.in_contextual_return_body();
    let inferred_expression = evaluate_return_expression_with_expected_type(
        expression,
        return_statement.expression_span,
        return_statement.span,
        return_type,
        symbols,
        ctx,
    );
    ctx.in_contextual_return_check = was_in_return_check;

    // An `any` return is what collapses tsc's inferred union, so the frame drops
    // its recorded verdicts when the body ends. The contextual evaluation above
    // cannot answer this — it reports per branch and yields the sentinel on a
    // mismatch — so ask the diagnostic-free inference path for the value's own
    // type, which for `cond ? anyValue : { … }` is the union tsc would form.
    if ctx.in_contextual_return_body()
        && may_infer_as_any(expression)
        && returns_any(&crate::infer::infer_expression(expression, symbols, ctx))
    {
        ctx.note_contextual_return_is_any();
    }

    match inferred_expression {
        InferredExpression::Known(source_type) => {
            ctx.note_contextual_return_type(&source_type);
            // A sentinel anywhere in either side means surge lost part of the
            // shape, so a mismatch reflects the modelling gap rather than the
            // source — the same deep guard the variable-declaration check
            // applies.
            if source_type.is_unknown() {
                return;
            }

            if !is_assignable_to(&source_type, &return_type) {
                // A sentinel anywhere in either side means surge lost part of
                // the shape, so the mismatch reflects the modelling gap rather
                // than the source — the same deep guard the variable
                // declaration check applies. Checked only after assignability
                // already failed: the deep walk forces lazy references, and
                // doing that on every clean return measurably perturbs later
                // resolutions (a false TS2554 on ky's `resolve()`).
                if crate::checks::function::type_contains_unknown(&source_type)
                    || crate::checks::function::type_contains_unknown(&return_type)
                {
                    return;
                }
                let reported_target =
                    crate::checks::expr::reported_relation_target(&source_type, &return_type);
                let source_type_name =
                    crate::checks::expr::source_display_name(&source_type, &reported_target);
                let target_type_name = reported_target.name();
                let diagnostic = crate::checks::expr::type_not_assignable_diagnostic(
                    &source_type,
                    &reported_target,
                    &source_type_name,
                    &target_type_name,
                    ctx.file_name.clone(),
                );

                let diagnostic = match return_statement.span.or(return_statement.expression_span) {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };

                // Same frame bookkeeping as the verdicts raised inside the
                // expression above: this is the whole-value one.
                let was_in_return_check = ctx.in_contextual_return_check;
                ctx.in_contextual_return_check = ctx.in_contextual_return_body();
                ctx.push(diagnostic);
                ctx.in_contextual_return_check = was_in_return_check;
            }
        }
        // The expected-type evaluation collapses to the sentinel once it has
        // reported a leaf mismatch, so the return's own type has to come from the
        // diagnostic-free path — it is what renders the whole signature tsc names
        // on the assignment. Only reached on a mismatch inside a contextually
        // typed body, so this costs nothing on a clean return.
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => {
            let mut noted = false;
            if ctx.in_contextual_return_body()
                && let InferredExpression::Known(source_type) =
                    crate::infer::infer_expression(expression, symbols, ctx)
            {
                ctx.note_contextual_return_type(&source_type);
                noted = true;
            }
            if !noted {
                ctx.note_contextual_return_type(&Type::Unknown);
            }
        }
    }
}
