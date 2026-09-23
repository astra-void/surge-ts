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

    // Every operand is visited even after one reports: tsc checks each
    // identifier on its own, so `a < b` reports both `a` and `b`.
    let mut blocked = false;
    let result = match expression {
        // The target is written, not read; the statement marks it assigned
        // once the expression has run.
        ParsedExpression::Assignment { value, value_span, .. } => check_expression_flow_impl(
            value,
            value_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
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
            blocked |= report_read_flow_positioned(
                callee_name,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
                true,
            ).is_blocked();

            for argument in arguments {
                blocked |= check_substituting_read_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::New {
            callee,
            callee_span,
            arguments,
            ..
        } => {
            blocked |= check_substituting_read_flow(
                callee,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

            for argument in arguments {
                blocked |= check_substituting_read_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyCall {
            object,
            object_span: _,
            arguments,
            ..
        } => {
            blocked |= check_substituting_read_flow(object, fallback_span, flow_state, statement_index, ctx).is_blocked();

            for argument in arguments {
                blocked |= check_substituting_read_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyAccess {
            object,
            object_span: _,
            ..
        } => check_substituting_read_flow(object, fallback_span, flow_state, statement_index, ctx),
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
        ParsedExpression::Update {
            operand,
            operand_span,
        }
        | ParsedExpression::Await {
            operand,
            operand_span,
        } => check_expression_flow_impl(
            operand,
            operand_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::ObjectRest { source, .. } => {
            check_expression_flow_impl(source, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::Sequence { expressions } => {
            for (expression, span) in expressions {
                blocked |= check_expression_flow_impl(
                    expression,
                    span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked();
            }
            FlowCheck::Clear
        }
        ParsedExpression::Binary {
            left,
            left_span,
            right,
            right_span,
            ..
        } => {
            blocked |= check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

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
            blocked |= check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

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
            ..
        } => {
            blocked |= check_expression_flow_impl(
                condition,
                condition_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

            blocked |= check_expression_flow_impl(
                when_true,
                when_true_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

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
                blocked |= check_expression_flow_impl(
                    &property.value,
                    property.value_span.or(property.span).or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            for element in elements {
                blocked |= check_expression_flow_impl(
                    &element.expression,
                    element.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => {
            blocked |= report_read_flow_positioned(
                object_name,
                object_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
                true,
            ).is_blocked();

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
            blocked |= check_substituting_read_flow(
                object,
                object_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ).is_blocked();

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
        // tsc reads a reference under a `NonNullExpression` with
        // `assumeInitialized`, so `resolve!` never reports TS2454 — the assertion
        // is the author stating the binding is set. Only the bare-identifier read
        // is exempted; any nested reads keep their own flow checks.
        ParsedExpression::NonNullAssertion {
            expression, span, ..
        } => match expression.as_ref() {
            ParsedExpression::Identifier { .. } => FlowCheck::Clear,
            expression => check_expression_flow_impl(
                expression,
                span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            ),
        },
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
            check_substituting_read_flow(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            let object_flow =
                check_substituting_read_flow(object, fallback_span, flow_state, statement_index, ctx);
            if object_flow.is_blocked() {
                return object_flow;
            }
            check_expression_flow_impl(index, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalPropertyCall { object, .. } => {
            check_substituting_read_flow(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalCall { callee, .. }
        | ParsedExpression::ExpressionCall { callee, .. } => {
            check_substituting_read_flow(callee, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            blocked |= check_expression_flow_impl(left, fallback_span, flow_state, statement_index, ctx).is_blocked();
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
                blocked |= report_read_flow(
                    name,
                    component_span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            for attribute in attributes {
                if let Some(value) = &attribute.value {
                    blocked |= check_expression_flow_impl(
                        value,
                        attribute.value_span.or(fallback_span),
                        flow_state,
                        statement_index,
                        ctx,
                    ).is_blocked();
                }
            }

            for child in children {
                blocked |= check_jsx_child_flow(child, fallback_span, flow_state, statement_index, ctx).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::JsxFragment { children, .. } => {
            for child in children {
                blocked |= check_jsx_child_flow(child, fallback_span, flow_state, statement_index, ctx).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::ArrowFunction(_) => FlowCheck::Clear,
        ParsedExpression::TemplateLiteral { expressions, .. } => {
            for expression in expressions {
                blocked |= check_expression_flow_impl(
                    expression,
                    fallback_span,
                    flow_state,
                    statement_index,
                    ctx,
                ).is_blocked();
            }

            FlowCheck::Clear
        }
        ParsedExpression::This { .. }
        | ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BigIntLiteral(_)
        | ParsedExpression::BooleanLiteral(_)
        | ParsedExpression::UndefinedLiteral
        | ParsedExpression::NullLiteral
        | ParsedExpression::TemplateStringsArray { .. }
        | ParsedExpression::Unknown => FlowCheck::Clear,
    };
    if blocked { FlowCheck::Blocked } else { result }
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
    ctx: &CheckerContext,
) {
    let variable_name = variable_name.into();
    if variable_kind == ParsedVariableKind::Var {
        // Hoisted when its container was entered; an initializer (or a type
        // that already admits `undefined`) settles it here, in whichever
        // branch the declaration sits.
        if has_initializer || declared_type.is_some_and(|ty| type_assumed_initialized(ty, ctx)) {
            flow_state.mark_assigned(&variable_name);
        }
        return;
    }
    if !matches!(
        variable_kind,
        ParsedVariableKind::Let | ParsedVariableKind::Const
    ) {
        return;
    }

    // tsc skips definite-assignment analysis for a binding whose declared type
    // already admits `undefined` — its `checkIdentifier` gate is
    // `AnyOrUnknown | Void`, plus any union containing `undefined`. Reading such
    // a binding before assignment yields `undefined`, which the type allows, so
    // no TS2454. Track it as already assigned so an unassigned read stays clear
    // (a use-before-declaration TDZ read is still caught by position).
    let state = if has_initializer || declared_type.is_some_and(|ty| type_assumed_initialized(ty, ctx)) {
        AssignmentState::Assigned
    } else {
        AssignmentState::DeclaredUnassigned
    };

    flow_state.declare_current(variable_name, state);
}

pub(crate) fn type_assumed_initialized(ty: &surge_ts_types::Type, ctx: &CheckerContext) -> bool {
    use surge_ts_types::Type;
    match ty {
        Type::Any | Type::Undefined | Type::Unknown | Type::GenuineUnknown | Type::Void => true,
        // tsc's gate looks at `T` itself, not its constraint, so a bare type
        // parameter is analyzed; the reads that see its constraint instead are
        // `FunctionFlowState::constraint_exempt`'s business.
        Type::TypeParameter(_) => false,
        Type::Union(union) => union.types().iter().any(|member| type_assumed_initialized(member, ctx)),
        _ => false,
    }
}

/// A read in a position where tsc substitutes a type parameter's union
/// constraint: the object of a member access, a callee, a call argument, or a
/// value under a non-generic contextual type. Only a bare identifier there is
/// such a read; anything else is walked as usual.
pub(crate) fn check_substituting_read_flow(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    match expression {
        ParsedExpression::Identifier { name, span } if flow_state.enabled && flow_state.tracked_local_count > 0 => {
            report_read_flow_positioned(
                name,
                span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
                true,
            )
        }
        _ => check_expression_flow_impl(expression, fallback_span, flow_state, statement_index, ctx),
    }
}

/// Whether a read of a binding typed by the in-scope type parameter `name` can
/// escape TS2454 by substitution: tsc substitutes a union or nullable
/// constraint (`isGenericTypeWithUnionConstraint`), and the substituted type
/// then passes the `containsUndefinedType` gate only if it carries `undefined`.
pub(crate) fn constraint_substitutes(name: &str, ctx: &CheckerContext) -> bool {
    fn union_or_nullable(constraint: &surge_ts_syntax::ParsedType, ctx: &CheckerContext, depth: usize) -> bool {
        use surge_ts_syntax::ParsedType;
        match constraint {
            ParsedType::Undefined => true,
            ParsedType::Union(members) => members
                .iter()
                .any(|member| union_or_nullable(member, ctx, depth + 1)),
            ParsedType::Named(named) if named.type_arguments.is_empty() && depth < 8 => {
                match ctx.type_declarations.get(&named.name) {
                    Some(crate::symbols::TypeDeclarationInfo::Alias(alias))
                        if alias.body.type_parameters.is_empty() =>
                    {
                        union_or_nullable(&alias.body.ty, ctx, depth + 1)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
    ctx.type_parameter_constraint(name)
        .is_some_and(|constraint| union_or_nullable(constraint, ctx, 0))
}

/// A contextual type with no top-level instantiable — what makes tsc
/// substitute a type parameter's constraint for the value read under it
/// (`hasContextualTypeWithNoGenericTypes`).
pub(crate) fn is_non_generic_contextual_type(ty: &surge_ts_types::Type) -> bool {
    use surge_ts_types::Type;
    match ty {
        Type::TypeParameter(_) | Type::Unknown | Type::GenuineUnknown | Type::Any | Type::ErrorType => false,
        Type::Union(union) => union.types().iter().all(is_non_generic_contextual_type),
        _ => true,
    }
}

/// The part of an assignment's value that reads bindings. The parser lowers
/// `x ??= v` (and `||=`, `&&=`) to `x = x ?? v`; tsc takes a logical assignment
/// as a definite assignment of `x`, not a read of it, so only `v` is checked.
pub(crate) fn assignment_value_read(
    assignment: &surge_ts_syntax::ParsedAssignment,
) -> (&ParsedExpression, Option<SyntaxTextSpan>) {
    match &assignment.value {
        ParsedExpression::NullishCoalescing {
            left,
            right,
            right_span,
            ..
        }
        | ParsedExpression::Logical {
            left,
            right,
            right_span,
            ..
        } if matches!(
            left.as_ref(),
            ParsedExpression::Identifier { name, span }
                if *name == assignment.target_name && *span == assignment.target_span
        ) =>
        {
            (right, *right_span)
        }
        value => (value, assignment.value_span),
    }
}

/// The assignments evaluating `expression` performs, innermost-first in
/// evaluation order (`a = b = c` assigns `b` before `a`), outside nested
/// functions, each flagged when it runs only on some paths through the
/// expression (the right of `&&`/`||`/`??`, a branch of `?:`).
pub(crate) fn expression_assignments(expression: &ParsedExpression) -> Vec<(&ParsedExpression, bool)> {
    fn collect<'a>(
        expression: &'a ParsedExpression,
        conditional: bool,
        found: &mut Vec<(&'a ParsedExpression, bool)>,
    ) {
        match expression {
            ParsedExpression::Logical { left, right, .. }
            | ParsedExpression::NullishCoalescing { left, right, .. } => {
                collect(left, conditional, found);
                collect(right, true, found);
            }
            ParsedExpression::Conditional {
                condition,
                when_true,
                when_false,
                ..
            } => {
                collect(condition, conditional, found);
                collect(when_true, true, found);
                collect(when_false, true, found);
            }
            _ => {
                expression.for_each_child(&mut |child| collect(child, conditional, found));
                if matches!(expression, ParsedExpression::Assignment { .. }) {
                    found.push((expression, conditional));
                }
            }
        }
    }
    let mut found = Vec::new();
    if expression.contains_assignment() {
        collect(expression, false, &mut found);
    }
    found
}

/// The assignments a condition has certainly run when it is true: an
/// `a && b` chain evaluates every operand before it is true.
pub(crate) fn condition_true_assignments(condition: &ParsedExpression) -> Vec<&ParsedExpression> {
    match condition {
        ParsedExpression::Logical {
            left,
            operator: surge_ts_syntax::ParsedLogicalOperator::And,
            right,
            ..
        } => {
            let mut found = condition_true_assignments(left);
            found.extend(condition_true_assignments(right));
            found
        }
        _ => expression_assignments(condition)
            .into_iter()
            .filter(|(_, conditional)| !conditional)
            .map(|(assignment, _)| assignment)
            .collect(),
    }
}

/// The bindings `expression` assigns on every path through it: both branches
/// of a `?:` count, the right of `&&`/`||`/`??` does not.
pub(crate) fn certainly_assigned_names(expression: &ParsedExpression) -> Vec<&str> {
    let mut names = Vec::new();
    if !expression.contains_assignment() {
        return names;
    }
    match expression {
        ParsedExpression::Assignment {
            target_name, value, ..
        } => {
            names.extend(certainly_assigned_names(value));
            names.push(target_name.as_str());
        }
        ParsedExpression::Logical { left, .. } | ParsedExpression::NullishCoalescing { left, .. } => {
            names.extend(certainly_assigned_names(left));
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            names.extend(certainly_assigned_names(condition));
            let on_false = certainly_assigned_names(when_false);
            names.extend(
                certainly_assigned_names(when_true)
                    .into_iter()
                    .filter(|name| on_false.contains(name)),
            );
        }
        _ => expression.for_each_child(&mut |child| names.extend(certainly_assigned_names(child))),
    }
    names
}

/// Marks the bindings `expression` certainly assigns as it runs, for definite
/// assignment after it.
pub(crate) fn mark_expression_assignments(
    expression: &ParsedExpression,
    flow_state: &mut FunctionFlowState,
) {
    for name in certainly_assigned_names(expression) {
        flow_state.mark_assigned(name);
    }
}
