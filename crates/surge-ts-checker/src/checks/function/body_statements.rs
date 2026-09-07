//! Per-statement checkers for function bodies (declarations, control flow,
//! assignments, returns) dispatched from [`super::check_function_body_statement`].

use super::*;

use surge_ts_diagnostics::{Diagnostic, DiagnosticCode};
use surge_ts_syntax::{
    ParsedAssignment, ParsedBindingName, ParsedExpression, ParsedForOfStatement,
    ParsedFunctionBodyStatement, ParsedIfStatement, ParsedMemberAssignment, ParsedReturnStatement,
    ParsedSwitchStatement, ParsedThisPropertyAssignment, ParsedTryStatement, ParsedType,
    ParsedUnaryOperator, ParsedVariableDeclaration, ParsedVariableKind, ParsedWhileStatement,
};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason};

use crate::checks::assign::check_assignment_with_symbols;
use crate::checks::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::checks::var::{VariableCheckOptions, check_variable_declaration_against_symbols};
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    AssignmentState, FlowCheck, FunctionFlowState, analyze_function_body_flow,
    apply_variable_declaration_state, check_assignment_target_flow, check_expression_flow,
    check_obvious_truthiness_condition, mark_assignment_state, merge_branch_deltas,
};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

pub(crate) fn check_function_variable_declaration(
    variable: ParsedVariableDeclaration,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let local_name = variable.name.clone();
    let variable_kind = variable.kind;
    let has_initializer = variable.initializer.is_some();
    // An ambient (`declare`) binding never carries an initializer but is not
    // "unassigned" — it is provided from outside. Body-local `enum`s lower to
    // one, so definite-assignment analysis must not report TS2454 on them. A
    // `let x!: T` definite-assignment assertion says the same thing explicitly.
    let definitely_assigned =
        has_initializer || variable.is_declare || variable.has_definite_assertion;

    // Track a boolean alias of a guard expression (`const ok = error &&
    // isError(error) && …`) so a later `if (!ok) return;` can narrow the guarded
    // identifiers in the fall-through, matching tsc's aliased-condition handling.
    if let Some(initializer) = variable.initializer.as_ref() {
        flow_state
            .record_alias_guard_targets(local_name.clone(), guarded_value_identifiers(initializer));
        // A `const` whose initializer is plainly a condition keeps that
        // condition, so a later `if (ok)` narrows exactly as the written
        // expression would (tsc's aliased-condition narrowing).
        if matches!(variable_kind, ParsedVariableKind::Const)
            && is_condition_shaped(initializer)
        {
            flow_state.record_alias_guard_condition(
                local_name.clone(),
                std::sync::Arc::new(initializer.clone()),
            );
        }
        // A `const` bound to a property reference is a discriminant alias:
        // `const { direction } = opts` lowers to `direction = opts.direction`,
        // and testing `direction` narrows `opts`.
        if matches!(variable_kind, ParsedVariableKind::Const)
            && is_property_reference(initializer)
        {
            flow_state.record_discriminant_alias(
                local_name.clone(),
                std::sync::Arc::new(initializer.clone()),
            );
        }
    }

    check_local_duplicate_declaration(&variable, scopes, ctx);

    let initializer_flow_blocked = variable.initializer.as_ref().is_some_and(|initializer| {
        if flow_state.tracked_local_count() == 0 {
            return false;
        }

        flow_state.begin_branch_capture();
        if matches!(
            variable_kind,
            ParsedVariableKind::Let | ParsedVariableKind::Const
        ) {
            flow_state.declare_current(local_name.as_str(), AssignmentState::DeclaredUnassigned);
        }

        let blocked = check_expression_flow(
            initializer,
            variable.initializer_span,
            flow_state,
            statement_index,
            ctx,
        )
        .is_blocked();
        let _ = flow_state.finish_branch_capture();
        blocked
    });

    // The declared name is visible inside its own initializer's nested function
    // bodies (`const t = setInterval(() => clearInterval(t), 10)`), which run
    // after the binding exists. It is seeded as the degradation sentinel so the
    // closure reference resolves without inventing a type; the real symbol
    // replaces it below. A *direct* self-read is still caught by the flow layer's
    // temporal-dead-zone check, which runs above.
    if has_initializer {
        scopes.insert_current(
            local_name.as_str(),
            SymbolInfo {
                ty: Type::Unknown,
                kind: symbol_kind_for_variable(variable_kind),
                function_signature: None,
            },
        );
    }

    let literal_initializer_type = matches!(
        variable_kind,
        ParsedVariableKind::Let | ParsedVariableKind::Var | ParsedVariableKind::Const
    )
    .then(|| literal_initializer_type(variable.initializer.as_ref()))
    .flatten();

    let visible_symbols = visible_symbols(scopes);

    if let Some(symbol) = check_variable_declaration_against_symbols(
        variable,
        visible_symbols,
        ctx,
        VariableCheckOptions {
            report_duplicate_let_const: false,
            check_initializer: !initializer_flow_blocked,
        },
    ) {
        apply_variable_declaration_state(
            variable_kind,
            local_name.as_str(),
            definitely_assigned,
            Some(&symbol.ty),
            flow_state,
        );
        // A declared union narrows to what the initializer can inhabit, exactly
        // as a later assignment does: `let style: Style = "simple"` is
        // `"simple"` until reassigned. Recorded as a *narrowing* so a later
        // assignment still checks against the declaration. Only a literal
        // initializer participates — its type is known without re-evaluating
        // (and re-reporting) the expression.
        let narrowed = literal_initializer_type.filter(|initialized| {
            matches!(symbol.ty, Type::Union(_)) && is_assignable_to(initialized, &symbol.ty)
        });
        match narrowed {
            Some(initialized) => {
                let declared = symbol.ty.clone();
                scopes.insert_current_handle(local_name.as_str(), symbol);
                scopes.insert_current_narrowed(
                    local_name.as_str(),
                    SymbolInfo {
                        ty: initialized,
                        kind: symbol_kind_for_variable(variable_kind),
                        function_signature: None,
                    },
                    declared,
                );
            }
            None => {
                scopes.insert_current_handle(local_name.as_str(), symbol);
            }
        }
    }
}

fn symbol_kind_for_variable(kind: ParsedVariableKind) -> SymbolKind {
    match kind {
        ParsedVariableKind::Let => SymbolKind::Let,
        ParsedVariableKind::Const => SymbolKind::Const,
        ParsedVariableKind::Var => SymbolKind::Var,
    }
}

/// The type of a literal initializer, which needs no expression evaluation (and
/// so cannot double-report the initializer's own diagnostics).
fn literal_initializer_type(initializer: Option<&ParsedExpression>) -> Option<Type> {
    match initializer? {
        ParsedExpression::StringLiteral(value) => Some(Type::StringLiteral(value.clone())),
        ParsedExpression::NumberLiteral(value) => Some(Type::NumberLiteral(
            surge_ts_types::NumberLiteralType {
                value: value.clone(),
            },
        )),
        ParsedExpression::BooleanLiteral(value) => Some(Type::BooleanLiteral(*value)),
        _ => None,
    }
}

pub(crate) fn check_function_block(
    block_body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    scopes.push_child();
    check_function_body(block_body, return_type, scopes, flow_state, ctx);
    scopes.pop_child();
}

/// Whether an initializer is plainly a boolean condition — a logical chain, a
/// negation, or a comparison. Deliberately narrow: a `const x = f()` is not
/// recorded, so the aliased-condition clone stays proportional to guard
/// aliases rather than to every `const` in the body.
fn is_condition_shaped(expression: &ParsedExpression) -> bool {
    use surge_ts_syntax::{ParsedBinaryOperator, ParsedUnaryOperator};
    match expression {
        ParsedExpression::Logical { .. } => true,
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            ..
        } => true,
        ParsedExpression::Binary { operator, .. } => matches!(
            operator,
            ParsedBinaryOperator::StrictEquals
                | ParsedBinaryOperator::Equals
                | ParsedBinaryOperator::StrictNotEquals
                | ParsedBinaryOperator::NotEquals
        ),
        _ => false,
    }
}

/// Narrows by a condition and, when it named a discriminant alias, by the
/// rewritten form as well. Both are applied: the written condition narrows the
/// alias binding itself (`if (transformer)` proves the local non-nullish), the
/// rewrite narrows the object it came from (`opts.transformer`).
fn narrow_condition_and_aliases_in_scope(
    base: &ParsedExpression,
    rewritten: Option<&ParsedExpression>,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) {
    narrow_discriminant_in_scope(base, scopes, branch_is_true, ctx);
    if let Some(rewritten) = rewritten {
        narrow_discriminant_in_scope(rewritten, scopes, branch_is_true, ctx);
    }
}

/// Whether an initializer is a static property reference over identifiers
/// (`opts.direction`, `node.kind.value`) — the shape a discriminant alias takes.
fn is_property_reference(expression: &ParsedExpression) -> bool {
    match expression {
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. } => {
            matches!(object.as_ref(), ParsedExpression::Identifier { .. })
                || is_property_reference(object)
        }
        _ => false,
    }
}

/// Substitutes discriminant aliases for the references they were bound to, so
/// `if (direction === "up")` narrows `opts` exactly as `opts.direction === "up"`
/// would. `None` when the condition names no alias.
fn rewrite_discriminant_aliases(
    condition: &ParsedExpression,
    flow_state: &FunctionFlowState,
) -> Option<ParsedExpression> {
    match condition {
        ParsedExpression::Identifier { name, .. } => flow_state.discriminant_alias(name).cloned(),
        ParsedExpression::Unary {
            operator,
            operator_span,
            operand,
            operand_span,
        } => {
            let rewritten = rewrite_discriminant_aliases(operand, flow_state)?;
            Some(ParsedExpression::Unary {
                operator: *operator,
                operator_span: *operator_span,
                operand: Box::new(rewritten),
                operand_span: *operand_span,
            })
        }
        ParsedExpression::Binary {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let new_left = rewrite_discriminant_aliases(left, flow_state);
            let new_right = rewrite_discriminant_aliases(right, flow_state);
            if new_left.is_none() && new_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Binary {
                left: Box::new(new_left.unwrap_or_else(|| left.as_ref().clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(new_right.unwrap_or_else(|| right.as_ref().clone())),
                right_span: *right_span,
            })
        }
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let new_left = rewrite_discriminant_aliases(left, flow_state);
            let new_right = rewrite_discriminant_aliases(right, flow_state);
            if new_left.is_none() && new_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Logical {
                left: Box::new(new_left.unwrap_or_else(|| left.as_ref().clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(new_right.unwrap_or_else(|| right.as_ref().clone())),
                right_span: *right_span,
            })
        }
        _ => None,
    }
}

/// The condition an `if` really tests: an identifier bound to a boolean `const`
/// guard stands for the expression it was initialized from.
fn resolved_alias_condition(
    condition: &ParsedExpression,
    flow_state: &FunctionFlowState,
) -> Option<std::sync::Arc<ParsedExpression>> {
    let ParsedExpression::Identifier { name, .. } = condition else {
        return None;
    };
    flow_state.alias_guard_condition(name)
}

/// Whether a branch body ends in a call to a `never`-returning function
/// (`process.exit(1)`), which ends control flow exactly as a `return` does.
/// [`analyze_function_body_flow`] is purely syntactic and cannot see a return
/// type, so this one type-dependent case is decided here, where the scope is in
/// hand — without it the fall-through of `if (!args.file) { …; process.exit(1); }`
/// keeps the unnarrowed `string | undefined`.
fn body_ends_in_never_call(body: &[ParsedFunctionBodyStatement], scopes: &ScopeStack) -> bool {
    match body.last() {
        Some(ParsedFunctionBodyStatement::Block(block)) => body_ends_in_never_call(block, scopes),
        Some(ParsedFunctionBodyStatement::Expression(expression)) => {
            call_returns_never(expression, scopes)
        }
        _ => false,
    }
}

/// Whether a call expression's callee is declared to return `never`.
fn call_returns_never(expression: &ParsedExpression, scopes: &ScopeStack) -> bool {
    let returns_never = |ty: &Type| {
        matches!(ty, Type::Function(function) if matches!(function.return_type(), Type::Never))
    };
    match expression {
        ParsedExpression::Call { callee_name, .. } => scopes
            .resolve(callee_name)
            .is_some_and(|symbol| returns_never(&symbol.ty)),
        ParsedExpression::PropertyCall {
            object,
            property_name,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return false;
            };
            scopes.resolve(name).is_some_and(|symbol| {
                symbol
                    .ty
                    .get_property_access_type(property_name)
                    .is_some_and(|ty| returns_never(&ty))
            })
        }
        _ => false,
    }
}

/// `if (!ok) <exit>` where `ok` is a boolean alias of a guard expression
/// narrows, in the fall-through, the identifiers that alias guarded — dropping a
/// guarded genuine-`unknown` to the degradation sentinel so a later access is
/// not a spurious `TS18046`. Mirrors tsc's aliased-condition narrowing, limited
/// to the genuine-unknown downgrade.
fn narrow_aliased_guard_after_exit(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    flow_state: &FunctionFlowState,
) {
    let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    else {
        return;
    };
    let ParsedExpression::Identifier { name, .. } = operand.as_ref() else {
        return;
    };
    if let Some(targets) = flow_state.alias_guard_targets(name) {
        let targets = targets.to_vec();
        downgrade_genuine_unknown_in_scope(&targets, scopes);
    }
}

/// The bindings a branch body assigns at its own statement level. Deeper
/// assignments are discarded with their own inner frame before the branch ends,
/// so they cannot reach the join.
fn branch_assigned_names(body: &[ParsedFunctionBodyStatement], names: &mut Vec<String>) {
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::Assignment(assignment) => {
                if !names.iter().any(|name| *name == assignment.target_name) {
                    names.push(assignment.target_name.clone());
                }
            }
            ParsedFunctionBodyStatement::Block(block) => branch_assigned_names(block, names),
            _ => {}
        }
    }
}

/// Joins a then-branch's end types with the fall-through (condition-false) types
/// for the bindings it assigned. tsc types the code after `if (!x) { x = … }`
/// from both incoming edges; surge's branch scope discards the assignment
/// narrowing on `pop_child`, which otherwise leaves the declared union in place
/// for every later use. The join only ever removes union members both edges
/// rule out — a widening result is a modelling artifact and is dropped.
fn join_branch_assignments(
    branch_types: &[(String, Type)],
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    if branch_types.is_empty() {
        return;
    }

    scopes.push_child();
    narrow_discriminant_in_scope(condition, scopes, false, ctx);
    let fallthrough_types: Vec<Option<Type>> = branch_types
        .iter()
        .map(|(name, _)| {
            scopes.resolve(name).map(|symbol| {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
            })
        })
        .collect();
    scopes.pop_child();

    for ((name, branch_ty), fallthrough_ty) in branch_types.iter().zip(fallthrough_types) {
        let Some(fallthrough_ty) = fallthrough_ty else {
            continue;
        };
        let joined = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
            union_type(vec![branch_ty.clone(), fallthrough_ty])
        });
        let Some(symbol) = scopes.resolve(name) else {
            continue;
        };
        // Bound by the *declaration*, not by whatever narrowing survives the
        // branch: with both edges narrowed (`let s: Wide = "a"; if (c) s = "b";`)
        // the join is legitimately wider than either, and comparing against the
        // fall-through narrowing alone would drop it.
        let bound = scopes
            .visible_symbols()
            .declared_type(name)
            .unwrap_or(&symbol.ty);
        if joined == symbol.ty || !is_assignable_to(&joined, bound) {
            continue;
        }
        let joined_symbol = SymbolInfo {
            ty: joined,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        // Written to the owning frame, not shadowed in the current one: the join
        // describes the binding from the `if` onward, and a block-local shadow
        // would be dropped before a `break`/loop-exit edge that carries it.
        let _ = scopes.update_visible(name, joined_symbol);
    }
}

/// Joins the two edges of an `if`/`else` for the bindings either branch assigns.
/// Both branch frames have popped, so each side's end type is supplied as a
/// snapshot; the result is bounded by the declaration, never by whatever
/// narrowing survives the statement.
fn join_branch_pair(
    then_types: &[(String, Type)],
    else_types: &[(String, Type)],
    scopes: &mut ScopeStack,
) {
    for (name, then_ty) in then_types {
        let Some((_, else_ty)) = else_types.iter().find(|(other, _)| other == name) else {
            continue;
        };
        let joined = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
            union_type(vec![then_ty.clone(), else_ty.clone()])
        });
        let Some(symbol) = scopes.resolve(name) else {
            continue;
        };
        let current = symbol.ty.clone();
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let bound = scopes
            .visible_symbols()
            .declared_type(name)
            .cloned()
            .unwrap_or_else(|| current.clone());
        if joined == current || !is_assignable_to(&joined, &bound) {
            continue;
        }
        let _ = scopes.update_visible(
            name,
            SymbolInfo {
                ty: joined,
                kind,
                function_signature,
            },
        );
    }
}

/// Snapshots the current type of each assigned binding at a branch's end, before
/// its scope frame pops.
fn branch_assignment_types(names: &[String], scopes: &ScopeStack) -> Vec<(String, Type)> {
    names
        .iter()
        .filter_map(|name| {
            scopes.resolve(name).map(|symbol| {
                (
                    name.clone(),
                    with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
                )
            })
        })
        .collect()
}

pub(crate) fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(&if_statement.condition, if_statement.condition_span, ctx);

    // `if (ok)` where `ok` is a boolean `const` alias narrows by the condition
    // the alias was written as, not by the opaque identifier.
    let alias_condition = resolved_alias_condition(&if_statement.condition, flow_state);
    let base_condition: &ParsedExpression =
        alias_condition.as_deref().unwrap_or(&if_statement.condition);
    let rewritten_condition = rewrite_discriminant_aliases(base_condition, flow_state);

    let then_flow = analyze_function_body_flow(&if_statement.then_body);
    let then_guarantees_value_return = then_flow.guarantees_value_return;
    // The code after `if (cond) <body>` sees `!cond` whenever the then-branch
    // cannot fall through — that includes `continue`/`break` (which only
    // `guarantees_exit` reports), not just a value `return`. Gating narrowing on
    // either keeps the old return-based behavior and adds early-`continue` guards.
    let then_diverts_control = then_guarantees_value_return
        || then_flow.guarantees_exit
        || body_ends_in_never_call(&if_statement.then_body, scopes);
    let has_else_body = !if_statement.else_body.is_empty();

    let else_flow_diverts = has_else_body && {
        let else_flow = analyze_function_body_flow(&if_statement.else_body);
        else_flow.guarantees_value_return
            || else_flow.guarantees_exit
            || body_ends_in_never_call(&if_statement.else_body, scopes)
    };
    let mut joinable_assignments = Vec::new();
    if !has_else_body && !then_diverts_control {
        branch_assigned_names(&if_statement.then_body, &mut joinable_assignments);
    } else if has_else_body && !then_diverts_control && !else_flow_diverts {
        // Both edges reach the join, so the binding is the union of what each
        // branch left it as — the fall-through edge the no-else form uses does
        // not exist here.
        branch_assigned_names(&if_statement.then_body, &mut joinable_assignments);
        branch_assigned_names(&if_statement.else_body, &mut joinable_assignments);
    }

    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &if_statement.condition,
            if_statement.condition_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_condition_expression_with_truthy_guards(
            &if_statement.condition,
            if_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    if flow_active {
        let mut branch_deltas = Vec::new();
        scopes.push_child();
        narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                true,
                ctx,
            );
        flow_state.begin_branch_capture();
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let mut then_delta = flow_state.finish_branch_capture();
        then_delta.continues = !then_diverts_control;
        let then_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        scopes.pop_child();
        if !has_else_body {
            join_branch_assignments(&then_assignment_types, base_condition, scopes, ctx);
        }
        branch_deltas.push(then_delta);

        if has_else_body {
            let else_flow = analyze_function_body_flow(&if_statement.else_body);
            let else_diverts_control =
                else_flow.guarantees_value_return || else_flow.guarantees_exit;
            scopes.push_child();
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                false,
                ctx,
            );
            flow_state.begin_branch_capture();
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            let mut else_delta = flow_state.finish_branch_capture();
            else_delta.continues = !else_diverts_control;
            let else_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
            scopes.pop_child();
            join_branch_pair(&then_assignment_types, &else_assignment_types, scopes);
            branch_deltas.push(else_delta);
        }

        if !has_else_body && then_diverts_control {
            narrow_truthy_guarded_identifiers(base_condition, scopes);
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                false,
                ctx,
            );
            narrow_aliased_guard_after_exit(&if_statement.condition, scopes, flow_state);
        }

        merge_branch_deltas(flow_state, &branch_deltas, !has_else_body);
    } else {
        scopes.push_child();
        narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                true,
                ctx,
            );
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let then_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        scopes.pop_child();
        if !has_else_body {
            join_branch_assignments(&then_assignment_types, base_condition, scopes, ctx);
        }

        if has_else_body {
            scopes.push_child();
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                false,
                ctx,
            );
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            let else_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
            scopes.pop_child();
            join_branch_pair(&then_assignment_types, &else_assignment_types, scopes);
        }

        if !has_else_body && then_diverts_control {
            narrow_truthy_guarded_identifiers(base_condition, scopes);
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                scopes,
                false,
                ctx,
            );
            narrow_aliased_guard_after_exit(&if_statement.condition, scopes, flow_state);
        }
    }
}

pub(crate) fn check_function_while_statement(
    while_statement: ParsedWhileStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(
        &while_statement.condition,
        while_statement.condition_span,
        ctx,
    );

    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &while_statement.condition,
            while_statement.condition_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_condition_expression_with_truthy_guards(
            &while_statement.condition,
            while_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    scopes.push_child();
    if flow_active {
        flow_state.begin_branch_capture();
        check_function_body(while_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(while_statement.body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
}

pub(crate) fn check_function_for_of_statement(
    for_of_statement: ParsedForOfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let flow_active = flow_state.tracked_local_count() > 0;
    let iterable_blocked = if flow_active {
        check_expression_flow(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    let mut element_type = Type::Unknown;
    if !iterable_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        if let InferredExpression::Known(iterable_type) = evaluate_expression(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            &visible_symbols,
            ctx,
        ) {
            element_type = for_of_element_type(&iterable_type);
        }
    }

    scopes.push_child();
    insert_binding_name(&for_of_statement.binding_name, element_type, scopes);
    if flow_active {
        flow_state.begin_branch_capture();
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
}

pub(crate) fn for_of_element_type(iterable_type: &Type) -> Type {
    match iterable_type {
        Type::Any => Type::Any,
        Type::Array(element) => with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
            element.as_ref().clone()
        }),
        Type::Tuple(elements) => {
            if elements.is_empty() {
                Type::Unknown
            } else {
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                    union_type(elements.clone())
                })
            }
        }
        Type::String | Type::StringLiteral(_) => Type::String,
        Type::Union(union) => {
            let element_types = union
                .types()
                .iter()
                .filter(|ty| **ty != Type::Undefined)
                .map(for_of_element_type)
                .collect::<Vec<_>>();

            if element_types.is_empty() {
                Type::Unknown
            } else {
                union_type(element_types)
            }
        }
        // A nominal collection/iterator reference (`Set<T>`, `Map<K, V>`,
        // `MapIterator<V>`, …) yields its element type from the resolved type
        // arguments, without forcing the whole lib iterator graph to expand.
        Type::Reference(reference) => {
            if let Some(element) = iterable_reference_element_type(reference) {
                element
            } else {
                // A non-collection reference may still be a structural iterable
                // (an array alias, a tuple alias). Peel once and re-derive; the
                // peeled shape is never another reference for these, so this does
                // not loop.
                match reference.resolve() {
                    Type::Reference(_) => Type::Unknown,
                    peeled => for_of_element_type(&peeled),
                }
            }
        }
        _ => Type::Unknown,
    }
}

/// The element type a `for…of` binds when iterating a known lib collection or
/// iterator reference, derived from its resolved type arguments. `Map`-like
/// references yield the `[K, V]` entry tuple; `Set`-like and the iterator
/// wrappers yield their single element argument. Returns `None` for any other
/// reference so the caller can fall back to structural peeling.
fn iterable_reference_element_type(reference: &surge_ts_types::TypeReference) -> Option<Type> {
    let name = reference.id.rsplit('\u{0}').next().unwrap_or(&reference.id);
    let arg = |index: usize| reference.arguments.get(index).cloned();
    match name {
        "Map" | "ReadonlyMap" | "WeakMap" => match (arg(0), arg(1)) {
            (Some(key), Some(value)) => Some(Type::Tuple(vec![key, value])),
            _ => Some(Type::Unknown),
        },
        "Set" | "ReadonlySet" | "WeakSet" => Some(arg(0).unwrap_or(Type::Unknown)),
        "IterableIterator"
        | "Iterator"
        | "IteratorObject"
        | "ArrayIterator"
        | "MapIterator"
        | "SetIterator"
        | "Generator"
        | "AsyncGenerator"
        | "IterableIteratorObject" => Some(arg(0).unwrap_or(Type::Unknown)),
        _ => None,
    }
}

/// TS7029 under `noFallthroughCasesInSwitch`: a non-empty clause whose end is
/// reachable falls through into the next clause. The last clause cannot fall
/// through, and empty clauses (stacked `case` labels) are allowed to.
fn emit_switch_fallthrough_diagnostics(
    switch_statement: &ParsedSwitchStatement,
    ctx: &mut CheckerContext,
) {
    let case_count = switch_statement.cases.len();
    for (index, case) in switch_statement.cases.iter().enumerate() {
        let is_last = index + 1 == case_count;
        if is_last || case.consequent.is_empty() {
            continue;
        }
        if !analyze_function_body_flow(&case.consequent).guarantees_exit {
            let diagnostic = Diagnostic::ts7029(ctx.file_name.clone());
            let diagnostic = match case.span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
    }
}

pub(crate) fn check_function_switch_statement(
    switch_statement: ParsedSwitchStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if ctx.options.no_fallthrough_cases_in_switch {
        emit_switch_fallthrough_diagnostics(&switch_statement, ctx);
    }

    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_expression(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            &visible_symbols,
            ctx,
        );
    }

    // `switch (x.kind) case "a":` narrows the case body exactly like
    // `if (x.kind === "a")`. A fall-through group (`case "a": case "b": body`)
    // narrows to the OR of the group's tests, matching tsc. Synthesize the
    // equality/OR condition and reuse the if-branch narrowing.
    let discriminant = switch_statement.discriminant.clone();
    let discriminant_span = switch_statement.discriminant_span;
    let equality_condition = |test: &ParsedExpression| ParsedExpression::Binary {
        left: Box::new(discriminant.clone()),
        left_span: discriminant_span,
        operator: surge_ts_syntax::ParsedBinaryOperator::StrictEquals,
        operator_span: None,
        right: Box::new(test.clone()),
        right_span: None,
    };
    // Per case: the tests of the maximal run of empty-consequent cases falling
    // into it, plus its own test. `None` for a group containing `default`.
    let case_group_conditions: Vec<Option<ParsedExpression>> = {
        let mut group: Vec<Option<&ParsedExpression>> = Vec::new();
        switch_statement
            .cases
            .iter()
            .map(|switch_case| {
                group.push(switch_case.test.as_ref());
                let condition = if group.iter().any(|test| test.is_none()) {
                    None
                } else {
                    group
                        .iter()
                        .filter_map(|test| *test)
                        .map(equality_condition)
                        .reduce(|left, right| ParsedExpression::Logical {
                            left: Box::new(left),
                            left_span: None,
                            operator: surge_ts_syntax::ParsedLogicalOperator::Or,
                            operator_span: None,
                            right: Box::new(right),
                            right_span: None,
                        })
                };
                if !switch_case.consequent.is_empty() {
                    group.clear();
                }
                condition
            })
            .collect()
    };

    if flow_active {
        let mut branch_deltas = Vec::new();

        for (case_index, switch_case) in switch_statement.cases.into_iter().enumerate() {
            let case_guarantees_value_return =
                analyze_function_body_flow(&switch_case.consequent).guarantees_value_return;
            if let Some(test) = switch_case.test.as_ref() {
                let _ = check_expression_flow(
                    test,
                    switch_case.test_span,
                    flow_state,
                    statement_index,
                    ctx,
                );
            }

            scopes.push_child();
            if let Some(condition) = case_group_conditions[case_index].as_ref() {
                narrow_discriminant_in_scope(condition, scopes, true, ctx);
            }
            flow_state.begin_branch_capture();
            check_function_body(
                switch_case.consequent,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            let mut case_delta = flow_state.finish_branch_capture();
            case_delta.continues = !case_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(case_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
    } else {
        for (case_index, switch_case) in switch_statement.cases.into_iter().enumerate() {
            scopes.push_child();
            if let Some(condition) = case_group_conditions[case_index].as_ref() {
                narrow_discriminant_in_scope(condition, scopes, true, ctx);
            }
            check_function_body(
                switch_case.consequent,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            scopes.pop_child();
        }
    }
}

pub(crate) fn check_function_try_statement(
    try_statement: ParsedTryStatement,
    _statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let flow_active = flow_state.tracked_local_count() > 0;

    if flow_active {
        let mut branch_deltas = Vec::new();
        let try_guarantees_value_return =
            analyze_function_body_flow(&try_statement.block).guarantees_value_return;
        scopes.push_child();
        flow_state.begin_branch_capture();
        check_function_body(
            try_statement.block,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let mut try_delta = flow_state.finish_branch_capture();
        try_delta.continues = !try_guarantees_value_return;
        scopes.pop_child();
        branch_deltas.push(try_delta);

        if let Some(handler_clause) = try_statement.handler {
            let catch_guarantees_value_return =
                analyze_function_body_flow(&handler_clause.body).guarantees_value_return;
            scopes.push_child();
            if let Some(binding_name) = handler_clause.binding_name.as_ref() {
                if let Some(declared_type) = handler_clause.declared_type.as_ref() {
                    if !matches!(
                        declared_type,
                        ParsedType::Any | ParsedType::Unknown | ParsedType::UnknownKeyword
                    ) {
                        let mut diagnostic = Diagnostic::new(
                            DiagnosticCode::TypeScript(1196),
                            "Catch clause variable type annotation must be 'any' or 'unknown' if specified.",
                            ctx.file_name.clone(),
                        );
                        if let ParsedBindingName::Identifier { span, .. } = binding_name {
                            if let Some(span) = span {
                                diagnostic = diagnostic.with_span(convert_span(*span));
                            }
                        }
                        ctx.push(diagnostic);
                    }
                }

                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(Type::Unknown);
                insert_binding_name(binding_name, catch_type, scopes);
            }
            flow_state.begin_branch_capture();
            check_function_body(
                handler_clause.body,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            let mut catch_delta = flow_state.finish_branch_capture();
            catch_delta.continues = !catch_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(catch_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
        scopes.push_child();
        check_function_body(
            try_statement.finalizer,
            return_type,
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();
    } else {
        scopes.push_child();
        check_function_body(
            try_statement.block,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();

        if let Some(handler_clause) = try_statement.handler {
            scopes.push_child();
            if let Some(binding_name) = handler_clause.binding_name.as_ref() {
                if let Some(declared_type) = handler_clause.declared_type.as_ref() {
                    if !matches!(
                        declared_type,
                        ParsedType::Any | ParsedType::Unknown | ParsedType::UnknownKeyword
                    ) {
                        let mut diagnostic = Diagnostic::new(
                            DiagnosticCode::TypeScript(1196),
                            "Catch clause variable type annotation must be 'any' or 'unknown' if specified.",
                            ctx.file_name.clone(),
                        );
                        if let ParsedBindingName::Identifier { span, .. } = binding_name {
                            if let Some(span) = span {
                                diagnostic = diagnostic.with_span(convert_span(*span));
                            }
                        }
                        ctx.push(diagnostic);
                    }
                }

                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(Type::Unknown);
                insert_binding_name(binding_name, catch_type, scopes);
            }
            check_function_body(
                handler_clause.body,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            scopes.pop_child();
        }

        scopes.push_child();
        check_function_body(
            try_statement.finalizer,
            return_type,
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();
    }
}

pub(crate) fn check_function_throw_statement(
    throw_statement: surge_ts_syntax::ParsedThrowStatement,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if flow_state.tracked_local_count() > 0 {
        let _ = check_expression_flow(
            &throw_statement.expression,
            throw_statement.expression_span,
            flow_state,
            statement_index,
            ctx,
        );
    }

    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(
        &throw_statement.expression,
        throw_statement.expression_span,
        &visible_symbols,
        ctx,
    );
}

pub(crate) fn check_function_assignment(
    assignment: ParsedAssignment,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let target_name = assignment.target_name.clone();

    let (target_blocked, value_blocked) = if flow_state.tracked_local_count() > 0 {
        (
            check_assignment_target_flow(
                &target_name,
                flow_state,
                statement_index,
                ctx,
                assignment.target_span,
            ),
            check_expression_flow(
                &assignment.value,
                assignment.value_span,
                flow_state,
                statement_index,
                ctx,
            ),
        )
    } else {
        (FlowCheck::Clear, FlowCheck::Clear)
    };

    if !target_blocked.is_blocked() && !value_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let inferred_value = evaluate_expression(
            &assignment.value,
            assignment.value_span,
            &visible_symbols,
            ctx,
        );
        check_assignment_with_symbols(assignment, &visible_symbols, ctx);
        update_assigned_symbol_type(&target_name, inferred_value, scopes);
    }

    if !target_blocked.is_blocked() && flow_state.tracked_local_count() > 0 {
        mark_assignment_state(&target_name, flow_state);
    }
}

/// Checks an `o.p = v` assignment against the target property's declared type
/// and narrows the target for the code that follows. Nothing is reported when
/// either side carries the degradation sentinel, and the narrowing is
/// block-scoped exactly like the identifier-assignment one above.
pub(crate) fn check_member_assignment(
    assignment: ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let ParsedExpression::PropertyAccess {
        object,
        object_span,
        property_name,
        ..
    } = &assignment.target
    else {
        return;
    };

    let visible_symbols = visible_symbols(scopes);

    // An assignment target is written, not read: it is resolved through the
    // *inference* layer, which answers without reporting, so a receiver surge
    // models incompletely (`Component.getInitialProps = …`, `X.prototype.m = …`)
    // does not turn into a false TS2339 here.
    let object_type = match crate::infer::infer_expression(object, &visible_symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        _ => return,
    };
    let _ = object_span;

    let Some(target_type) = object_type.get_property_access_type(property_name) else {
        return;
    };

    let inferred_value = crate::checks::expected::evaluate_expression_with_expected_type(
        &assignment.value,
        assignment.value_span,
        Some(&target_type),
        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
        &visible_symbols,
        ctx,
    );

    let InferredExpression::Known(value_type) = inferred_value else {
        return;
    };

    if value_type.is_unknown() || target_type.is_unknown() {
        return;
    }

    if !is_assignable_to(&value_type, &target_type) {
        let diagnostic = Diagnostic::ts2322(
            &crate::checks::expr::source_display_name(&value_type, &target_type),
            &target_type.name(),
            ctx.file_name.clone(),
        );
        let diagnostic = match assignment.target_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return;
    }

    crate::checks::function::narrowing::narrow_assignment_target_in_scope(
        &assignment.target,
        &value_type,
        scopes,
    );
}

/// Checks a `this.<property> = <value>` assignment against the instance
/// property's declared type. The `this` symbol is bound to the class instance
/// type for the duration of the method/constructor body. When `this` or the
/// property cannot be resolved, no diagnostic is emitted so unsupported class
/// shapes do not cascade.
pub(crate) fn check_this_property_assignment(
    assignment: ParsedThisPropertyAssignment,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);

    let Some(this_symbol) = visible_symbols.get("this") else {
        return;
    };

    let Some(property_type) = this_symbol
        .ty
        .get_property_access_type(&assignment.property_name)
    else {
        return;
    };

    let inferred_value = evaluate_expression(
        &assignment.value,
        assignment.value_span,
        &visible_symbols,
        ctx,
    );

    let InferredExpression::Known(value_type) = inferred_value else {
        return;
    };

    if value_type.is_unknown() || property_type.is_unknown() {
        return;
    }

    if !is_assignable_to(&value_type, &property_type) {
        let diagnostic = Diagnostic::ts2322(
            &crate::checks::expr::source_display_name(&value_type, &property_type),
            &property_type.name(),
            ctx.file_name.clone(),
        );
        let diagnostic = match assignment.value_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

pub(crate) fn update_assigned_symbol_type(
    target_name: &str,
    inferred_value: InferredExpression,
    scopes: &mut ScopeStack,
) {
    let InferredExpression::Known(value_ty) = inferred_value else {
        return;
    };

    if value_ty.is_unknown() {
        return;
    }

    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };

    let mut narrowed_by_assignment = false;
    let updated_ty = if symbol.ty == Type::Undefined {
        union_type(vec![
            Type::Undefined,
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || value_ty.clone()),
        ])
    } else if symbol.ty == value_ty || is_assignable_to(&value_ty, &symbol.ty) {
        // Assigning to a union-declared variable narrows it to what was
        // assigned, as tsc does: the lazy-singleton idiom
        // (`let client: Redis | null = null; … client = new Redis(); return client;`)
        // otherwise keeps reading as the full union at every later use.
        if matches!(symbol.ty, Type::Union(_)) && !value_ty.is_unknown() {
            narrowed_by_assignment = true;
            value_ty
        } else {
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
        }
    } else if matches!(symbol.ty, Type::Any | Type::Unknown | Type::GenuineUnknown) {
        union_type(vec![
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
            value_ty,
        ])
    } else if scopes
        .visible_symbols()
        .declared_type(target_name)
        .is_some_and(|declared| {
            matches!(declared, Type::Union(_)) && is_assignable_to(&value_ty, declared)
        })
    {
        // The binding is already narrowed (by its initializer, or by an earlier
        // assignment) to something this value does not inhabit. The assignment is
        // still legal against the *declaration*, and it re-narrows to the new
        // value — `let s: Wide = "a"; s = "c";` is `"c"`, not a rejected write.
        narrowed_by_assignment = true;
        value_ty
    } else {
        // Preserve the declared/inferred symbol type when an incompatible assignment
        // is already reported to avoid cascading return/usage diagnostics.
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    };

    if updated_ty == symbol.ty {
        return;
    }

    let updated = SymbolInfo {
        ty: updated_ty,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };

    if narrowed_by_assignment {
        // Assignment narrowing is block-scoped: written into the current frame it
        // is discarded when a branch scope pops, so `if (t === "draft-4") t = "draft-04";`
        // leaves the declared union in place for the code that follows.
        scopes.insert_current(target_name, updated);
        return;
    }

    let _ = scopes.update_visible(target_name, updated);
}

pub(crate) fn check_function_expression_statement(
    expression: ParsedExpression,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if let ParsedExpression::Conditional {
        condition,
        condition_span,
        ..
    } = &expression
    {
        check_obvious_truthiness_condition(condition, *condition_span, ctx);
    }

    let flow_blocked = if flow_state.tracked_local_count() > 0 {
        check_expression_flow(&expression, None, flow_state, statement_index, ctx)
    } else {
        FlowCheck::Clear
    };

    if flow_blocked.is_blocked() {
        return;
    }

    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(&expression, None, &visible_symbols, ctx);
}

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
fn may_infer_as_any(expression: &ParsedExpression) -> bool {
    !matches!(
        expression,
        ParsedExpression::ObjectLiteral { .. }
            | ParsedExpression::ArrayLiteral { .. }
            | ParsedExpression::ArrowFunction(_)
            | ParsedExpression::JsxElement { .. }
            | ParsedExpression::JsxFragment { .. }
            | ParsedExpression::StringLiteral(_)
            | ParsedExpression::NumberLiteral(_)
            | ParsedExpression::BooleanLiteral(_)
            | ParsedExpression::TemplateLiteral { .. }
            | ParsedExpression::UndefinedLiteral
            | ParsedExpression::NullLiteral
    )
}

/// Whether a return value's own type is `any`, directly or as a union member —
/// the shape that collapses tsc's inferred return type. See
/// `ContextualReturnFrame`.
fn returns_any(inferred: &InferredExpression) -> bool {
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
        if let InferredExpression::Known(source_type) = inferred {
            ctx.note_contextual_return_type(&source_type);
        }
        return;
    };

    // Only the mismatch verdicts raised while checking this value belong to the
    // contextual-return frame; everything else the expression reports is
    // unrelated and must survive.
    let was_in_return_check = ctx.in_contextual_return_check;
    ctx.in_contextual_return_check = ctx.in_contextual_return_body();
    let inferred_expression = evaluate_expression_with_expected_type(
        expression,
        return_statement.expression_span,
        Some(return_type),
        ExpectedTypeDiagnostic::TypeNotAssignable,
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
                let source_type_name =
                    crate::checks::expr::source_display_name(&source_type, &return_type);
                let target_type_name = return_type.name();
                let diagnostic =
                    Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

                let diagnostic = match return_statement.expression_span {
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
            if ctx.in_contextual_return_body()
                && let InferredExpression::Known(source_type) =
                    crate::infer::infer_expression(expression, symbols, ctx)
            {
                ctx.note_contextual_return_type(&source_type);
            }
        }
    }
}
