//! Expression-level flow: assignment targets, declaration state, truthiness conditions.

use super::*;

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedExpression, ParsedJsxChild, ParsedVariableKind, TextSpan as SyntaxTextSpan,
};

use crate::context::{CheckerContext, convert_span};
use crate::program::{
    record_flow_expression_visit_count, record_flow_identifier_read_count,
    record_flow_truthiness_check_count,
};

pub(crate) fn check_expression_flow_impl(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    record_flow_expression_visit_count();
    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return FlowCheck::Clear;
    }

    match expression {
        ParsedExpression::Identifier { name, span } => report_read_flow(
            name,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::Call {
            callee_name,
            callee_span,
            arguments,
            ..
        } => {
            if report_read_flow(
                callee_name,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if check_expression_flow_impl(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::New {
            callee,
            callee_span,
            arguments,
            ..
        } => {
            if check_expression_flow_impl(
                callee,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if check_expression_flow_impl(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyCall {
            object,
            object_span: _,
            arguments,
            ..
        } => {
            if check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if check_expression_flow_impl(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyAccess {
            object,
            object_span: _,
            ..
        } => check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx),
        ParsedExpression::Unary {
            operand,
            operand_span,
            ..
        } => check_expression_flow_impl(
            operand,
            operand_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::Binary {
            left,
            left_span,
            right,
            right_span,
            ..
        } => {
            if check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                right,
                right_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::Logical {
            left,
            left_span,
            right,
            right_span,
            ..
        } => {
            if check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                right,
                right_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
        } => {
            if check_expression_flow_impl(
                condition,
                condition_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            if check_expression_flow_impl(
                when_true,
                when_true_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                when_false,
                when_false_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                if check_expression_flow_impl(
                    &property.value,
                    property.value_span.or(property.span).or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            for element in elements {
                if check_expression_flow_impl(
                    &element.expression,
                    element.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => {
            if report_read_flow(
                object_name,
                object_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                index,
                index_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::ElementAccess {
            object,
            object_span,
            index,
            index_span,
        } => {
            if check_expression_flow_impl(
                object,
                object_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                index,
                index_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::TypeAssertion {
            expression,
            expression_span,
            ..
        } => check_expression_flow_impl(
            expression,
            expression_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::SatisfiesExpression {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::NonNullAssertion {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::ConstAssertion {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::OptionalPropertyAccess { object, .. } => {
            check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            let object_flow =
                check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx);
            if object_flow.is_blocked() {
                return object_flow;
            }
            check_expression_flow_impl(index, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalPropertyCall { object, .. } => {
            check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalCall { callee, .. } => {
            check_expression_flow_impl(callee, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            if check_expression_flow_impl(left, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }
            check_expression_flow_impl(right, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::JsxElement {
            component_name,
            component_span,
            attributes,
            children,
            ..
        } => {
            if let Some(name) = component_name {
                if report_read_flow(
                    name,
                    component_span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            for attribute in attributes {
                if let Some(value) = &attribute.value {
                    if check_expression_flow_impl(
                        value,
                        attribute.value_span.or(fallback_span),
                        flow_state,
                        statement_index,
                        ctx,
                    )
                    .is_blocked()
                    {
                        return FlowCheck::Blocked;
                    }
                }
            }

            for child in children {
                if check_jsx_child_flow(child, fallback_span, flow_state, statement_index, ctx)
                    .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::JsxFragment { children, .. } => {
            for child in children {
                if check_jsx_child_flow(child, fallback_span, flow_state, statement_index, ctx)
                    .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::ArrowFunction(_) => FlowCheck::Clear,
        ParsedExpression::This { .. }
        | ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BooleanLiteral(_)
        | ParsedExpression::UndefinedLiteral
        | ParsedExpression::NullLiteral
        | ParsedExpression::Unknown => FlowCheck::Clear,
    }
}

fn check_jsx_child_flow(
    child: &ParsedJsxChild,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    match child {
        ParsedJsxChild::Text => FlowCheck::Clear,
        ParsedJsxChild::Expression { expression, span } => match expression {
            Some(expression) => check_expression_flow_impl(
                expression,
                span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ),
            None => FlowCheck::Clear,
        },
        ParsedJsxChild::Element(element) => {
            check_expression_flow_impl(element, fallback_span, flow_state, statement_index, ctx)
        }
    }
}

pub(crate) fn check_assignment_target_flow(
    target_name: &str,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
    span: Option<SyntaxTextSpan>,
) -> FlowCheck {
    record_flow_identifier_read_count();
    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return FlowCheck::Clear;
    }

    let FlowReadOutcome::UseBeforeDeclaration =
        flow_state.read_identifier(target_name, statement_index)
    else {
        return FlowCheck::Clear;
    };

    let mut diagnostic = Diagnostic::ts2448(target_name, ctx.file_name.clone());
    if let Some(span) = span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
    FlowCheck::Blocked
}

pub(crate) fn apply_variable_declaration_state(
    variable_kind: surge_ts_syntax::ParsedVariableKind,
    variable_name: impl Into<Arc<str>>,
    has_initializer: bool,
    declared_type: Option<&surge_ts_types::Type>,
    flow_state: &mut FunctionFlowState,
) {
    if !matches!(
        variable_kind,
        ParsedVariableKind::Let | ParsedVariableKind::Const
    ) {
        return;
    }

    // tsc skips definite-assignment analysis for a binding whose declared type
    // already permits `undefined` (`any`, or any union containing `undefined`):
    // reading it before assignment yields `undefined`, which the type allows, so
    // no TS2454. Track such a binding as already assigned so an unassigned read
    // stays clear (a use-before-declaration TDZ read is still caught by position).
    let state = if has_initializer || declared_type.is_some_and(type_permits_undefined) {
        AssignmentState::Assigned
    } else {
        AssignmentState::DeclaredUnassigned
    };

    flow_state.declare_current(variable_name, state);
}

fn type_permits_undefined(ty: &surge_ts_types::Type) -> bool {
    use surge_ts_types::Type;
    match ty {
        Type::Any | Type::Undefined => true,
        Type::Union(union) => union.types().iter().any(type_permits_undefined),
        _ => false,
    }
}

pub(crate) fn check_obvious_truthiness_condition(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    record_flow_truthiness_check_count();
    // This is intentionally narrow: it only covers syntax the project already parses
    // and only emits the obvious truthiness diagnostics that the current checker supports.
    let (diagnostic, diagnostic_emitted) = match expression {
        ParsedExpression::StringLiteral(_) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::NumberLiteral(value)
            if value.parse::<f64>().map_or(false, |n| n == 0.0) =>
        {
            (Diagnostic::ts2873(ctx.file_name.clone()), false)
        }
        ParsedExpression::NumberLiteral(_) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::BooleanLiteral(true) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::BooleanLiteral(false) => {
            (Diagnostic::ts2873(ctx.file_name.clone()), false)
        }
        ParsedExpression::UndefinedLiteral | ParsedExpression::NullLiteral => return false,
        _ => return false,
    };

    let diagnostic = match fallback_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };

    ctx.push(diagnostic);
    diagnostic_emitted
}
