use surge_ts_diagnostics::{Diagnostic, DiagnosticCode};
use surge_ts_syntax::{
    ParsedExpression, ParsedForOfStatement, ParsedIfStatement, ParsedSwitchStatement,
    ParsedTryStatement, ParsedType, ParsedWhileStatement,
};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use super::super::narrowing::narrow_reference_non_null_in_scope;
use super::super::{
    check_function_body, evaluate_condition_expression_with_truthy_guards, insert_binding_name,
    narrow_discriminant_in_scope, narrow_truthy_guarded_identifiers, visible_symbols,
};
use super::{
    adopt_branch_assignments, apply_condition_true_assignment_types, apply_expression_assignments,
    body_ends_in_never_call, branch_assigned_names, branch_assignment_types, deep_assigned_names,
    join_branch_assignments, join_branch_edges, join_branch_pair, loop_join_names,
    mark_condition_true_assignments, narrow_aliased_guard_after_exit,
    narrow_condition_and_aliases_in_scope, narrow_tuple_destructure_siblings, prime_loop_mutations,
    release_loop_mutations, resolved_alias_condition, rewrite_discriminant_aliases,
    widen_assigned_bindings, widen_loop_assigned_bindings,
};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    FlowCheck, FunctionFlowState, analyze_function_body_flow, check_expression_flow,
    merge_branch_deltas,
};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::ScopeStack;

/// TS2774: a condition that tests a function for truthiness, and never calls or
/// otherwise mentions it where the test holds, is always true — tsc's
/// `checkTestingKnownTruthyType`. Only a type that is certainly callable counts:
/// a union (an optional method, `F | undefined`) has no call signature of its own.
pub(crate) fn report_unreferenced_callable_conditions(
    tests: &[surge_ts_syntax::ParsedTruthinessTest],
    symbols: &crate::symbols::SymbolTable,
    ctx: &mut CheckerContext,
) {
    for test in tests {
        let InferredExpression::Known(ty) =
            crate::infer::infer_expression(&test.expression, symbols, ctx)
        else {
            continue;
        };
        let callable = match ty.peeled() {
            Type::Function(_) => true,
            Type::Object(object) => object.call_signature().is_some(),
            _ => false,
        };
        if callable && let Some(span) = test.span {
            ctx.push(Diagnostic::ts2774(ctx.file_name.clone()).with_span(convert_span(span)));
        }
    }
}

pub(crate) fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    report_unreferenced_callable_conditions(
        &if_statement.unreferenced_truthiness_tests,
        &visible_symbols(scopes),
        ctx,
    );
    // `if (ok)` where `ok` is a `const` alias of a condition narrows by the
    // condition the alias was written as — and, in each branch below, by `ok`
    // itself as well: `const perf = inBrowser && source; if (perf)` leaves
    // `perf` truthy, whatever it says about `inBrowser` and `source`.
    // An assignment in the condition has run by the time it is tested:
    // `if ((x = f()))` narrows `x`.
    let narrowing_source = if_statement
        .condition
        .contains_assignment()
        .then(|| if_statement.condition.with_assignments_as_reads());
    let narrowing_condition = narrowing_source.as_ref().unwrap_or(&if_statement.condition);
    let alias_condition = resolved_alias_condition(narrowing_condition, flow_state);
    let base_condition: &ParsedExpression =
        alias_condition.as_deref().unwrap_or(narrowing_condition);
    let rewritten_condition = rewrite_discriminant_aliases(base_condition, flow_state);
    // The identifier an alias condition replaced: it is narrowed by the test
    // itself, not only by the expression it stands for.
    let alias_source = alias_condition.as_ref().map(|_| narrowing_condition);

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
    // tsc's post-`if` flow node joins the branches that can complete. When only
    // one can, the code after the `if` sees exactly what that branch left: its
    // condition narrowing, the fall-through of an `else if` nested in it, and
    // whatever it assigned. The no-else form below is the same rule with an
    // empty surviving branch.
    let only_then_completes = has_else_body && else_flow_diverts && !then_diverts_control;
    let only_else_completes = has_else_body && then_diverts_control && !else_flow_diverts;
    let mut surviving_narrowings = Vec::new();
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
    apply_expression_assignments(&if_statement.condition, scopes, flow_state, ctx);

    if flow_active {
        let mut branch_deltas = Vec::new();
        scopes.push_child();
        apply_condition_true_assignment_types(&if_statement.condition, scopes, ctx);
        narrow_condition_and_aliases_in_scope(
            base_condition,
            rewritten_condition.as_ref(),
            alias_source,
            scopes,
            true,
            ctx,
        );
        if alias_condition.is_some() {
            narrow_discriminant_in_scope(&if_statement.condition, scopes, true, ctx);
        }
        flow_state.begin_branch_capture();
        mark_condition_true_assignments(&if_statement.condition, flow_state);
        crate::flow::mark_condition_defined(&if_statement.condition, true, flow_state, ctx);
        let then_unreachable = crate::flow::condition_never_takes(&if_statement.condition, true);
        flow_state.enter_unreachable(then_unreachable);
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        flow_state.exit_unreachable(then_unreachable);
        let mut then_delta = flow_state.finish_branch_capture();
        // tsc folds only the `true`/`false` keywords out of the flow graph.
        let condition_always_true = matches!(
            if_statement.condition,
            ParsedExpression::BooleanLiteral(true)
        );
        let condition_always_false = matches!(
            if_statement.condition,
            ParsedExpression::BooleanLiteral(false)
        );
        then_delta.continues = !then_diverts_control && !condition_always_false;
        let then_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        if only_then_completes {
            surviving_narrowings = scopes.current_frame_narrowings();
        }
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
                alias_source,
                scopes,
                false,
                ctx,
            );
            if alias_condition.is_some() {
                narrow_discriminant_in_scope(&if_statement.condition, scopes, false, ctx);
            }
            flow_state.begin_branch_capture();
            crate::flow::mark_condition_defined(&if_statement.condition, false, flow_state, ctx);
            let else_unreachable =
                crate::flow::condition_never_takes(&if_statement.condition, false);
            flow_state.enter_unreachable(else_unreachable);
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            flow_state.exit_unreachable(else_unreachable);
            let mut else_delta = flow_state.finish_branch_capture();
            else_delta.continues = !else_diverts_control && !condition_always_true;
            let else_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
            if only_else_completes {
                surviving_narrowings = scopes.current_frame_narrowings();
            }
            scopes.pop_child();
            join_branch_pair(&then_assignment_types, &else_assignment_types, scopes);
            branch_deltas.push(else_delta);
        }

        scopes.adopt_narrowings(std::mem::take(&mut surviving_narrowings));

        if !has_else_body && then_diverts_control {
            narrow_truthy_guarded_identifiers(base_condition, scopes);
            if let Some(alias_source) = alias_source {
                narrow_truthy_guarded_identifiers(alias_source, scopes);
            }
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                alias_source,
                scopes,
                false,
                ctx,
            );
            if alias_condition.is_some() {
                narrow_discriminant_in_scope(&if_statement.condition, scopes, false, ctx);
            }
            narrow_aliased_guard_after_exit(&if_statement.condition, scopes, flow_state);
        }

        // Without an `else`, the condition's false edge is the other way to
        // the join, carrying what that edge proves defined.
        if !has_else_body {
            flow_state.begin_branch_capture();
            crate::flow::mark_condition_defined(&if_statement.condition, false, flow_state, ctx);
            let mut fallthrough_delta = flow_state.finish_branch_capture();
            fallthrough_delta.continues = !condition_always_true;
            branch_deltas.push(fallthrough_delta);
        }
        merge_branch_deltas(flow_state, &branch_deltas, false);
    } else {
        scopes.push_child();
        narrow_condition_and_aliases_in_scope(
            base_condition,
            rewritten_condition.as_ref(),
            alias_source,
            scopes,
            true,
            ctx,
        );
        if alias_condition.is_some() {
            narrow_discriminant_in_scope(&if_statement.condition, scopes, true, ctx);
        }
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let then_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        if only_then_completes {
            surviving_narrowings = scopes.current_frame_narrowings();
        }
        scopes.pop_child();
        if !has_else_body {
            join_branch_assignments(&then_assignment_types, base_condition, scopes, ctx);
        }

        if has_else_body {
            scopes.push_child();
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                alias_source,
                scopes,
                false,
                ctx,
            );
            if alias_condition.is_some() {
                narrow_discriminant_in_scope(&if_statement.condition, scopes, false, ctx);
            }
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            let else_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
            if only_else_completes {
                surviving_narrowings = scopes.current_frame_narrowings();
            }
            scopes.pop_child();
            join_branch_pair(&then_assignment_types, &else_assignment_types, scopes);
        }

        scopes.adopt_narrowings(std::mem::take(&mut surviving_narrowings));

        if !has_else_body && then_diverts_control {
            narrow_truthy_guarded_identifiers(base_condition, scopes);
            if let Some(alias_source) = alias_source {
                narrow_truthy_guarded_identifiers(alias_source, scopes);
            }
            narrow_condition_and_aliases_in_scope(
                base_condition,
                rewritten_condition.as_ref(),
                alias_source,
                scopes,
                false,
                ctx,
            );
            if alias_condition.is_some() {
                narrow_discriminant_in_scope(&if_statement.condition, scopes, false, ctx);
            }
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
    let ParsedWhileStatement {
        condition,
        condition_span,
        body,
        runs_at_least_once,
    } = while_statement;

    // A lowered `do … while (c)` runs its body first, so the body is checked
    // with the enclosing flow state (its assignments stand afterwards) and the
    // condition is checked last, where it may read what the body assigned.
    // tsc types the code after a loop from every edge into it: the body's end
    // (the assignments it made) and, unless the body always runs, the state
    // before the first iteration.
    let assigned = loop_join_names(&body);
    widen_loop_assigned_bindings(&body, return_type, scopes, flow_state, ctx);
    if runs_at_least_once {
        scopes.push_child();
        let pending_mutations = prime_loop_mutations(&body, scopes, ctx);
        check_function_body(body, return_type, scopes, flow_state, ctx);
        release_loop_mutations(pending_mutations, scopes, ctx);
        let body_types = branch_assignment_types(&assigned, scopes);
        scopes.pop_child();
        adopt_branch_assignments(&body_types, scopes);
        check_while_condition(
            &condition,
            condition_span,
            statement_index,
            scopes,
            flow_state,
            ctx,
        );
        return;
    }

    check_while_condition(
        &condition,
        condition_span,
        statement_index,
        scopes,
        flow_state,
        ctx,
    );

    let entry_types = branch_assignment_types(&assigned, scopes);
    scopes.push_child();
    // The condition is tested before every iteration, so the body starts where
    // it held, whatever the previous iteration assigned.
    // An assignment in the condition has run by the time it is tested:
    // `while ((m = re.exec(s)))` narrows `m`.
    let narrowing_source = condition
        .contains_assignment()
        .then(|| condition.with_assignments_as_reads());
    let narrowing_condition = narrowing_source.as_ref().unwrap_or(&condition);
    let alias_condition = resolved_alias_condition(narrowing_condition, flow_state);
    let base_condition: &ParsedExpression =
        alias_condition.as_deref().unwrap_or(narrowing_condition);
    let rewritten_condition = rewrite_discriminant_aliases(base_condition, flow_state);
    // The identifier an alias condition replaced: it is narrowed by the test
    // itself, not only by the expression it stands for.
    let alias_source = alias_condition.as_ref().map(|_| narrowing_condition);
    apply_condition_true_assignment_types(&condition, scopes, ctx);
    narrow_condition_and_aliases_in_scope(
        base_condition,
        rewritten_condition.as_ref(),
        alias_source,
        scopes,
        true,
        ctx,
    );
    if alias_condition.is_some() {
        narrow_discriminant_in_scope(&condition, scopes, true, ctx);
    }
    let pending_mutations = prime_loop_mutations(&body, scopes, ctx);
    if flow_state.tracked_local_count() > 0 {
        flow_state.begin_branch_capture();
        mark_condition_true_assignments(&condition, flow_state);
        check_function_body(body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(body, return_type, scopes, flow_state, ctx);
    }
    release_loop_mutations(pending_mutations, scopes, ctx);
    let body_types = branch_assignment_types(&assigned, scopes);
    scopes.pop_child();
    join_branch_pair(&entry_types, &body_types, scopes);
}

fn check_while_condition(
    condition: &surge_ts_syntax::ParsedExpression,
    condition_span: Option<surge_ts_syntax::TextSpan>,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let condition_blocked = if flow_state.tracked_local_count() > 0 {
        check_expression_flow(condition, condition_span, flow_state, statement_index, ctx)
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_condition_expression_with_truthy_guards(
            condition,
            condition_span,
            &visible_symbols,
            ctx,
        );
    }
    apply_expression_assignments(condition, scopes, flow_state, ctx);
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
    let head = crate::flow::for_head_initializing(&for_of_statement);
    let iterable_blocked = if flow_active || !head.is_empty() {
        let mark = flow_state.begin_initializer(head);
        let blocked = check_expression_flow(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            flow_state,
            statement_index,
            ctx,
        );
        flow_state.end_initializer(mark);
        blocked
    } else {
        FlowCheck::Clear
    };

    // A `let`/`const` head is declared by the statement, so the expression it
    // iterates already sees it — only a deferred read can get past the
    // temporal dead zone reported above, and its type is circular there.
    let head_scoped =
        for_of_statement.binding_kind == surge_ts_syntax::ParsedForBindingKind::BlockScoped;
    if head_scoped {
        scopes.push_child();
        insert_binding_name(&for_of_statement.binding_name, Type::Any, scopes);
    }
    let mut element_type = Type::Unknown;
    let mut numeric_property_names = false;
    if !iterable_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let iterable_type = evaluate_expression(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            &visible_symbols,
            ctx,
        );
        // `for (k in o)` binds the property key, which is always `string` — the
        // right-hand side is still evaluated so errors inside it surface. The
        // key is *indexed* as a number when the iterated object's only index
        // signature is numeric, which is a property of the access rather than
        // of the binding (tsc: `isForInVariableForNumericPropertyNames`).
        if for_of_statement.keys_only {
            if let InferredExpression::Known(iterable_type) = &iterable_type {
                check_for_in_right_operand(
                    &for_of_statement.iterable,
                    iterable_type,
                    for_of_statement.iterable_span,
                    ctx,
                );
            }
            // Over a type variable tsc binds the generic `Extract<keyof T,
            // string>` (`getIndexTypeOrString`), which a write through it
            // defers on rather than landing on an index signature; surge has
            // no such type, and over a shape it could not model it cannot tell.
            element_type = match &iterable_type {
                InferredExpression::Known(iterable_type)
                    if iterable_type.is_type_variable() || iterable_type.is_unmodelled() =>
                {
                    Type::Unknown
                }
                _ => Type::String,
            };
            numeric_property_names = matches!(
                &iterable_type,
                InferredExpression::Known(iterable_type)
                    if has_numeric_property_names(iterable_type)
            );
        } else {
            crate::checks::expr::check_property_receiver(
                &for_of_statement.iterable,
                &iterable_type,
                for_of_statement.iterable_span,
                None,
                &visible_symbols,
                ctx,
            );
            // Without `strictNullChecks`, `unknown` reaches the iteration
            // protocol check instead of `checkNonNullType`.
            if !surge_ts_types::strict_null_checks()
                && matches!(
                    iterable_type,
                    InferredExpression::Known(Type::GenuineUnknown)
                )
            {
                ctx.push(crate::spans::diagnostic_with_syntax_span(
                    Diagnostic::ts2488("unknown", ctx.file_name.clone()),
                    for_of_statement.iterable_span,
                ));
            }
            if !for_of_statement.is_await {
                crate::checks::expr::check_iterable_operand(
                    &iterable_type,
                    for_of_statement.iterable_span,
                    false,
                    ctx,
                );
            }
            if let InferredExpression::Known(iterable_type) = iterable_type {
                element_type = for_of_element_type(&iterable_type);
            }
        }
    }
    if head_scoped {
        scopes.pop_child();
    }

    let assigned = loop_join_names(&for_of_statement.body);
    scopes.push_child();
    // `for (const _ in ref)` acts as a non-null assertion on `ref` for the
    // duration of the body (tsc: `getTypeAtFlowNode`, flow.go — "for (const _
    // in ref) acts as a nonnull on ref"), so indexing an optional object with
    // the key it just produced is not a possibly-undefined access.
    if for_of_statement.keys_only {
        narrow_reference_non_null_in_scope(&for_of_statement.iterable, scopes);
    }
    let loop_assigned_name = match (
        &for_of_statement.binding_name,
        for_of_statement.binding_kind,
    ) {
        (
            surge_ts_syntax::ParsedBindingName::Identifier { name, .. },
            surge_ts_syntax::ParsedForBindingKind::Var
            | surge_ts_syntax::ParsedForBindingKind::ExistingBinding,
        ) => Some(name.clone()),
        _ => None,
    };
    // A hoisted `var` whose element type already admits `undefined` is never
    // reported (tsc's `containsUndefinedType` gate), however the loop runs.
    if for_of_statement.binding_kind == surge_ts_syntax::ParsedForBindingKind::Var
        && let Some(name) = &loop_assigned_name
        && crate::flow::type_assumed_initialized(&element_type, ctx)
    {
        flow_state.mark_assigned(name);
    }
    insert_binding_name(&for_of_statement.binding_name, element_type, scopes);
    // A `var` head is function-scoped: it stays visible after the loop.
    if for_of_statement.binding_kind == surge_ts_syntax::ParsedForBindingKind::Var
        && let Some(name) = &loop_assigned_name
        && let Some(symbol) = scopes.resolve(name).cloned()
    {
        scopes.insert_current(
            name.as_str(),
            crate::symbols::SymbolInfo {
                kind: crate::symbols::SymbolKind::Var,
                ..symbol
            },
        );
    }
    // The key of a `for…in` over a numerically-keyed object indexes as a
    // number, so an access through it reads the numeric index signature rather
    // than reporting an implicit `any`.
    if numeric_property_names
        && let surge_ts_syntax::ParsedBindingName::Identifier { name, .. } =
            &for_of_statement.binding_name
        && let Some(symbol) = scopes.resolve(name).cloned()
    {
        scopes.insert_current(
            name.as_str(),
            crate::symbols::SymbolInfo {
                kind: crate::symbols::SymbolKind::ForInNumericKey,
                ..symbol
            },
        );
    }
    // The pre-pass reads the loop binding, so it runs once that is in scope;
    // head types land on the declaring frames, outside this child.
    widen_loop_assigned_bindings(&for_of_statement.body, return_type, scopes, flow_state, ctx);
    let entry_types = branch_assignment_types(&assigned, scopes);
    let pending_mutations = prime_loop_mutations(&for_of_statement.body, scopes, ctx);
    if flow_active {
        flow_state.begin_branch_capture();
        // Each iteration assigns the head before the body runs; the loop may
        // run zero times, so this stays inside the body's branch.
        if let Some(name) = &loop_assigned_name {
            flow_state.mark_assigned(name);
        }
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
    }
    release_loop_mutations(pending_mutations, scopes, ctx);
    let body_types = branch_assignment_types(&assigned, scopes);
    scopes.pop_child();
    join_branch_pair(&entry_types, &body_types, scopes);
}

/// tsc's `checkForInStatement` right-hand rule (TS2407): with `null` and
/// `undefined` removed, the operand must be `any`, an object type or a type
/// parameter. The `null` keyword leaves nothing, so it reads as `never`.
fn check_for_in_right_operand(
    operand: &surge_ts_syntax::ParsedExpression,
    operand_type: &Type,
    span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) {
    fn is_object_like(ty: &Type) -> bool {
        match ty.peeled() {
            Type::Any
            | Type::Object(_)
            | Type::Function(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::OpenTuple(_) => true,
            Type::Union(union) => union.types().iter().all(is_object_like),
            _ => false,
        }
    }
    let right_type = if matches!(operand, surge_ts_syntax::ParsedExpression::NullLiteral) {
        Type::Never
    } else {
        let peeled = operand_type.peeled();
        let degraded =
            |ty: &Type| matches!(ty, Type::Unknown | Type::ErrorType | Type::TypeParameter(_));
        if degraded(&peeled)
            || matches!(&peeled, Type::Union(union) if union.types().iter().any(degraded))
        {
            return;
        }
        match peeled {
            Type::Undefined => Type::Never,
            _ => surge_ts_types::remove_undefined(operand_type),
        }
    };
    if right_type != Type::Never && is_object_like(&right_type) {
        return;
    }
    let span = span
        .and_then(|span| ctx.parenthesized_outer_span(span))
        .or(span);
    ctx.push(crate::spans::diagnostic_with_syntax_span(
        Diagnostic::ts2407(right_type.name(), ctx.file_name.clone()),
        span,
    ));
}

/// tsc's `hasNumericPropertyNames`: the type's only index signature is the
/// numeric one, which is what makes a `for…in` key over it a `number`.
fn has_numeric_property_names(ty: &Type) -> bool {
    match ty.peeled() {
        Type::Object(object) => object.has_numeric_property_names(),
        // An array or tuple is indexed by number and nothing else, which is the
        // whole point of the rule: `for (const k in xs) xs[k]` is the idiom it
        // exists for.
        Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) => true,
        _ => false,
    }
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
                .filter(|ty| !matches!(ty, Type::Undefined | Type::Null))
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
pub(super) fn iterable_reference_element_type(
    reference: &surge_ts_types::TypeReference,
) -> Option<Type> {
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
pub(super) fn emit_switch_fallthrough_diagnostics(
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

/// A `case` test is an expression of its own — whatever is written there is
/// checked — and its type has to be comparable to the discriminant's, the same
/// relation `===` uses (TS2678). A `default` clause has no test.
fn check_switch_case_tests(
    switch_statement: &ParsedSwitchStatement,
    discriminant: &InferredExpression,
    symbols: &crate::symbols::SymbolTable,
    ctx: &mut CheckerContext,
) {
    // The comparison uses the discriminant's *declared* type when it has one.
    // A `let` the source narrows through a closure keeps its declared union in
    // tsc, and surge's flow state can be narrower than the source really
    // guarantees — comparing against the narrowed type there reports a case the
    // program does reach.
    let declared_discriminant = match &switch_statement.discriminant {
        ParsedExpression::Identifier { name, .. } => symbols.declared_type(name),
        _ => None,
    };

    for switch_case in &switch_statement.cases {
        let Some(test) = switch_case.test.as_ref() else {
            continue;
        };
        let inferred_case = evaluate_expression(test, switch_case.test_span, symbols, ctx);
        let (Some(case_type), Some(discriminant_type)) = (
            crate::checks::ops::inferred_type(&inferred_case),
            declared_discriminant.or_else(|| crate::checks::ops::inferred_type(discriminant)),
        ) else {
            continue;
        };
        // Same exemptions the `===` check makes: a degraded or `any` operand
        // says nothing, and a nullish test is comparable to anything.
        if case_type.is_unknown()
            || discriminant_type.is_unknown()
            || matches!(case_type, Type::Any | Type::Undefined | Type::Null)
            || matches!(discriminant_type, Type::Any | Type::Undefined | Type::Null)
        {
            continue;
        }
        if crate::checks::ops::types_overlap_for_equality(case_type, discriminant_type) {
            continue;
        }
        // A relation error, not TS2367's pairing: the discriminant is named as
        // declared and the case widens only against a literal-free target.
        let case_name = crate::checks::expr::source_display_name(case_type, discriminant_type);
        let discriminant_name = discriminant_type.name();
        let diagnostic = Diagnostic::ts2678(&case_name, &discriminant_name, ctx.file_name.clone());
        let diagnostic = match switch_case.test_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

pub(crate) fn check_function_switch_statement(
    mut switch_statement: ParsedSwitchStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    // A template without substitutions is a string literal to tsc, as a case
    // test too (`case \`number\`:` under `switch (typeof x)`).
    for case in &mut switch_statement.cases {
        if let Some(ParsedExpression::TemplateLiteral {
            expressions,
            quasis,
            ..
        }) = &case.test
            && expressions.is_empty()
            && let [Some(text)] = quasis.as_slice()
        {
            case.test = Some(ParsedExpression::StringLiteral(text.clone()));
        }
    }
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
        let discriminant = evaluate_expression(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            &visible_symbols,
            ctx,
        );
        check_switch_case_tests(&switch_statement, &discriminant, &visible_symbols, ctx);
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
    let any_of = |tests: &mut dyn Iterator<Item = &ParsedExpression>| {
        tests
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
    // Matching none of the cases is what the `default` clause, and the code
    // after a `switch` without one, sees (`narrowTypeBySwitchOnDiscriminant`).
    let no_case_matches = any_of(
        &mut switch_statement
            .cases
            .iter()
            .filter_map(|case| case.test.as_ref()),
    );
    // Per case: the tests of the maximal run of empty-consequent cases falling
    // into it, plus its own test, and the branch of that condition the body
    // runs in. A group containing `default` runs wherever no case *outside* the
    // group matched.
    let case_group_conditions: Vec<Option<(ParsedExpression, bool)>> = {
        let mut group: Vec<Option<&ParsedExpression>> = Vec::new();
        switch_statement
            .cases
            .iter()
            .map(|switch_case| {
                group.push(switch_case.test.as_ref());
                let condition = if group.iter().any(|test| test.is_none()) {
                    any_of(&mut switch_statement.cases.iter().filter_map(|case| {
                        case.test.as_ref().filter(|test| {
                            !group
                                .iter()
                                .flatten()
                                .any(|member| std::ptr::eq(*member, *test))
                        })
                    }))
                    .map(|condition| (condition, false))
                } else {
                    any_of(&mut group.iter().filter_map(|test| *test))
                        .map(|condition| (condition, true))
                };
                if !switch_case.consequent.is_empty() {
                    group.clear();
                }
                condition
            })
            .collect()
    };
    let has_default = switch_statement
        .cases
        .iter()
        .any(|case| case.test.is_none());
    let case_literals = (!has_default)
        .then(|| switch_case_literals(&switch_statement.cases, scopes, ctx))
        .flatten();
    let every_case_exits = switch_statement
        .cases
        .last()
        .is_some_and(|case| !case.consequent.is_empty())
        && switch_statement
            .cases
            .iter()
            .filter(|case| !case.consequent.is_empty())
            .all(|case| {
                // `break` leaves the case for the code after the `switch`.
                let flow = analyze_function_body_flow(&case.consequent);
                (flow.guarantees_value_return || flow.guarantees_exit)
                    && !crate::flow::body_breaks_enclosing_loop(&case.consequent)
            });

    // Every case that reaches the code after the `switch` (by `break` or by
    // running off its end) is an edge into it, and so is matching no case when
    // there is no `default`; the bindings the cases assign are joined over them.
    let mut assigned = Vec::new();
    for switch_case in &switch_statement.cases {
        branch_assigned_names(&switch_case.consequent, &mut assigned);
    }
    let case_reaches_end: Vec<bool> = switch_statement
        .cases
        .iter()
        .map(|switch_case| {
            let flow = analyze_function_body_flow(&switch_case.consequent);
            !flow.guarantees_value_return
                && (!flow.guarantees_exit
                    || crate::flow::body_breaks_enclosing_loop(&switch_case.consequent))
        })
        .collect();
    let mut edges = Vec::new();
    if !has_default && !assigned.is_empty() {
        edges.push(branch_assignment_types(&assigned, scopes));
    }

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
            if let Some((condition, branch_is_true)) = case_group_conditions[case_index].as_ref() {
                narrow_discriminant_in_scope(condition, scopes, *branch_is_true, ctx);
                narrow_tuple_destructure_siblings(condition, scopes, *branch_is_true);
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
            if !assigned.is_empty() && case_reaches_end[case_index] {
                edges.push(branch_assignment_types(&assigned, scopes));
            }
            scopes.pop_child();
            branch_deltas.push(case_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
    } else {
        for (case_index, switch_case) in switch_statement.cases.into_iter().enumerate() {
            scopes.push_child();
            if let Some((condition, branch_is_true)) = case_group_conditions[case_index].as_ref() {
                narrow_discriminant_in_scope(condition, scopes, *branch_is_true, ctx);
                narrow_tuple_destructure_siblings(condition, scopes, *branch_is_true);
            }
            check_function_body(
                switch_case.consequent,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            if !assigned.is_empty() && case_reaches_end[case_index] {
                edges.push(branch_assignment_types(&assigned, scopes));
            }
            scopes.pop_child();
        }
    }
    join_branch_edges(&edges, scopes);

    // Only the implicit `default` path continues past a `switch` whose every
    // case leaves it.
    if !has_default
        && every_case_exits
        && let Some(condition) = no_case_matches.as_ref()
    {
        record_non_exhaustive_switch(
            &switch_statement.discriminant,
            switch_statement.span,
            case_literals.as_deref(),
            scopes,
            ctx,
        );
        narrow_discriminant_in_scope(condition, scopes, false, ctx);
        narrow_tuple_destructure_siblings(condition, scopes, false);
    }
}

/// tsc's `isExhaustiveSwitchStatement`, recorded only on positive evidence: a
/// discriminant typed as a union of literals (or `boolean`) with a member no
/// case names, or an open primitive no finite set of cases covers. Any case test
/// that is not a literal, or a discriminant of another kind, records nothing.
fn record_non_exhaustive_switch(
    discriminant: &ParsedExpression,
    span: Option<surge_ts_syntax::TextSpan>,
    case_literals: Option<&[Type]>,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    let (Some(span), Some(case_literals)) = (span, case_literals) else {
        return;
    };
    // `switch (typeof x)` is exhaustive when its cases name every tag `x` can
    // have, not every tag `typeof` can produce.
    if let ParsedExpression::Unary {
        operator: surge_ts_syntax::ParsedUnaryOperator::Typeof,
        operand,
        ..
    } = discriminant
    {
        let InferredExpression::Known(operand_type) =
            crate::infer::infer_expression(operand, &visible_symbols(scopes), ctx)
        else {
            return;
        };
        let Some(tags) = crate::checks::function::narrowing::typeof_tags_of(&operand_type) else {
            return;
        };
        if tags
            .iter()
            .any(|tag| !case_literals.contains(&Type::StringLiteral((*tag).to_string())))
        {
            ctx.non_exhaustive_switches.push((span.start, span.end));
        }
        return;
    }
    let InferredExpression::Known(discriminant_type) =
        crate::infer::infer_expression(discriminant, &visible_symbols(scopes), ctx)
    else {
        return;
    };
    let members: Vec<Type> = match discriminant_type.peeled() {
        // A discriminant surge could not type is one tsc usually can; its end
        // point is left unreported rather than guessed reachable.
        Type::Any | Type::Unknown | Type::ErrorType => {
            ctx.exhaustive_switches.push((span.start, span.end));
            return;
        }
        Type::String | Type::Number | Type::BigInt => {
            ctx.non_exhaustive_switches.push((span.start, span.end));
            return;
        }
        Type::Boolean => vec![Type::BooleanLiteral(true), Type::BooleanLiteral(false)],
        Type::Union(union) => {
            let mut members = Vec::new();
            for member in union.types() {
                match member {
                    Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
                        members.push(member.clone())
                    }
                    Type::Boolean => {
                        members.push(Type::BooleanLiteral(true));
                        members.push(Type::BooleanLiteral(false));
                    }
                    _ => return,
                }
            }
            members
        }
        _ => return,
    };
    if members.iter().any(|member| !case_literals.contains(member)) {
        ctx.non_exhaustive_switches.push((span.start, span.end));
    } else {
        ctx.exhaustive_switches.push((span.start, span.end));
    }
}

/// The literal each case tests — written, or the unit type of a `const` it
/// names (`case STATUS_DONE:`) — or `None` when any case tests something else.
fn switch_case_literals(
    cases: &[surge_ts_syntax::ParsedSwitchCase],
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) -> Option<Vec<Type>> {
    cases
        .iter()
        .filter_map(|case| case.test.as_ref())
        .map(|test| match test {
            ParsedExpression::StringLiteral(value) => Some(Type::StringLiteral(value.clone())),
            ParsedExpression::NumberLiteral(value) => {
                Some(Type::NumberLiteral(surge_ts_types::NumberLiteralType {
                    value: value.clone(),
                }))
            }
            ParsedExpression::BooleanLiteral(value) => Some(Type::BooleanLiteral(*value)),
            ParsedExpression::Identifier { .. } | ParsedExpression::PropertyAccess { .. } => {
                let diagnostics_before = ctx.diagnostics().len();
                let inferred = crate::infer::infer_expression(test, &visible_symbols(scopes), ctx);
                ctx.truncate_diagnostics(diagnostics_before);
                match inferred {
                    InferredExpression::Known(
                        literal @ (Type::StringLiteral(_)
                        | Type::NumberLiteral(_)
                        | Type::BooleanLiteral(_)),
                    ) => Some(literal),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

/// Undoes the widening the handler was checked under, except for what the
/// handler itself assigned: that stands at the handler's end.
fn restore_after_catch(
    before_catch: &[(String, Type)],
    catch_body: &[surge_ts_syntax::ParsedFunctionBodyStatement],
    scopes: &mut ScopeStack,
) {
    let catch_assigned = deep_assigned_names(&[catch_body]);
    let catch_end = branch_assignment_types(&catch_assigned, scopes);
    adopt_branch_assignments(before_catch, scopes);
    adopt_branch_assignments(&catch_end, scopes);
}

/// A `finally` block runs however the `try` and `catch` ended — part-way
/// included — so a binding either assigns is read at its declared type there.
/// The types the join left for the code after the statement come back once it
/// is checked.
fn check_finalizer(
    finalizer: Vec<surge_ts_syntax::ParsedFunctionBodyStatement>,
    assigning_bodies: &[&[surge_ts_syntax::ParsedFunctionBodyStatement]],
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if finalizer.is_empty() {
        return;
    }
    let assigned = deep_assigned_names(assigning_bodies);
    let after_try = branch_assignment_types(&assigned, scopes);
    widen_assigned_bindings(assigning_bodies, scopes);
    scopes.push_child();
    check_function_body(finalizer, return_type, scopes, flow_state, ctx);
    scopes.pop_child();
    adopt_branch_assignments(&after_try, scopes);
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
        let try_flow = analyze_function_body_flow(&try_statement.block);
        let try_guarantees_value_return = try_flow.guarantees_value_return;
        // A branch that returns — with a value or bare — or throws does not
        // reach the code after the `try`.
        let try_exits = try_guarantees_value_return || try_flow.guarantees_exit;
        // Reaching the code after a `try` whose handler returns or throws means
        // the block completed, so its assignments hold from there on.
        let handler_diverts = try_statement.handler.as_ref().is_none_or(|handler| {
            let flow = analyze_function_body_flow(&handler.body);
            flow.guarantees_value_return
                || flow.guarantees_exit
                || body_ends_in_never_call(&handler.body, scopes)
        });
        let mut joinable_assignments = Vec::new();
        if handler_diverts && !try_guarantees_value_return {
            branch_assigned_names(&try_statement.block, &mut joinable_assignments);
        }
        let try_block_for_widening = try_statement.block.clone();
        // A handler that completes is an edge of its own into the code after
        // the statement: joined with the block's, or alone when the block
        // cannot complete.
        let mut edge_assignments = Vec::new();
        if !handler_diverts {
            branch_assigned_names(&try_statement.block, &mut edge_assignments);
            if let Some(handler) = try_statement.handler.as_ref() {
                branch_assigned_names(&handler.body, &mut edge_assignments);
            }
        }
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
        try_delta.continues = !try_exits;
        let try_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        let try_edge_types = branch_assignment_types(&edge_assignments, scopes);
        let mut catch_edge_types = Vec::new();
        scopes.pop_child();
        branch_deltas.push(try_delta);

        let try_block_assigned = deep_assigned_names(&[&try_block_for_widening]);
        let catch_body_for_widening = try_statement
            .handler
            .as_ref()
            .map(|handler| handler.body.clone())
            .unwrap_or_default();
        if let Some(handler_clause) = try_statement.handler {
            let catch_flow = analyze_function_body_flow(&handler_clause.body);
            let catch_exits = catch_flow.guarantees_value_return
                || catch_flow.guarantees_exit
                || body_ends_in_never_call(&handler_clause.body, scopes);
            // The handler can be entered from any point in the block.
            let before_catch = branch_assignment_types(&try_block_assigned, scopes);
            widen_assigned_bindings(&[&try_block_for_widening], scopes);
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
                        if let Some(span) = handler_clause.declared_type_span {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }
                        ctx.push(diagnostic);
                    }
                }

                // tsc types an unannotated catch variable `unknown` under
                // `strict` (`useUnknownInCatchVariables`), and rejects it as a
                // source for any parameter that is not `unknown`/`any`.
                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(if ctx.options.use_unknown_in_catch_variables {
                        Type::GenuineUnknown
                    } else {
                        Type::Any
                    });
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
            catch_delta.continues = !catch_exits;
            catch_edge_types = branch_assignment_types(&edge_assignments, scopes);
            scopes.pop_child();
            branch_deltas.push(catch_delta);
            restore_after_catch(&before_catch, &catch_body_for_widening, scopes);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
        adopt_branch_assignments(&try_assignment_types, scopes);
        join_handler_edge(try_exits, try_edge_types, catch_edge_types, scopes);
        check_finalizer(
            try_statement.finalizer,
            &[&try_block_for_widening, &catch_body_for_widening],
            return_type,
            scopes,
            flow_state,
            ctx,
        );
    } else {
        // The same join the flow-active path performs. It is not conditional on
        // definite-assignment tracking: reaching the code after a `try` whose
        // handler diverts means the block completed, so what it narrowed holds
        // — and a body with no tracked locals still narrows (`context.response
        // = await fetch(…)` inside `try`, read after it, in ofetch).
        let handler_diverts = try_statement.handler.as_ref().is_none_or(|handler| {
            let flow = analyze_function_body_flow(&handler.body);
            flow.guarantees_value_return
                || flow.guarantees_exit
                || body_ends_in_never_call(&handler.body, scopes)
        });
        let mut joinable_assignments = Vec::new();
        if handler_diverts
            && !analyze_function_body_flow(&try_statement.block).guarantees_value_return
        {
            branch_assigned_names(&try_statement.block, &mut joinable_assignments);
        }
        let try_exits = {
            let flow = analyze_function_body_flow(&try_statement.block);
            flow.guarantees_value_return || flow.guarantees_exit
        };
        let mut edge_assignments = Vec::new();
        if !handler_diverts {
            branch_assigned_names(&try_statement.block, &mut edge_assignments);
            if let Some(handler) = try_statement.handler.as_ref() {
                branch_assigned_names(&handler.body, &mut edge_assignments);
            }
        }

        let try_block_for_widening = try_statement.block.clone();
        let catch_body_for_widening = try_statement
            .handler
            .as_ref()
            .map(|handler| handler.body.clone())
            .unwrap_or_default();
        let try_block_assigned = deep_assigned_names(&[&try_block_for_widening]);
        scopes.push_child();
        check_function_body(
            try_statement.block,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let try_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
        let try_edge_types = branch_assignment_types(&edge_assignments, scopes);
        let mut catch_edge_types = Vec::new();
        scopes.pop_child();
        adopt_branch_assignments(&try_assignment_types, scopes);

        if let Some(handler_clause) = try_statement.handler {
            // The handler can be entered from any point in the block.
            let before_catch = branch_assignment_types(&try_block_assigned, scopes);
            widen_assigned_bindings(&[&try_block_for_widening], scopes);
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
                        if let Some(span) = handler_clause.declared_type_span {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }
                        ctx.push(diagnostic);
                    }
                }

                // tsc types an unannotated catch variable `unknown` under
                // `strict` (`useUnknownInCatchVariables`), and rejects it as a
                // source for any parameter that is not `unknown`/`any`.
                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(if ctx.options.use_unknown_in_catch_variables {
                        Type::GenuineUnknown
                    } else {
                        Type::Any
                    });
                insert_binding_name(binding_name, catch_type, scopes);
            }
            check_function_body(
                handler_clause.body,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            catch_edge_types = branch_assignment_types(&edge_assignments, scopes);
            scopes.pop_child();
            restore_after_catch(&before_catch, &catch_body_for_widening, scopes);
        }
        join_handler_edge(try_exits, try_edge_types, catch_edge_types, scopes);

        check_finalizer(
            try_statement.finalizer,
            &[&try_block_for_widening, &catch_body_for_widening],
            return_type,
            scopes,
            flow_state,
            ctx,
        );
    }
}

/// The code after a `try` whose handler completes: reached from the handler
/// alone when the block cannot complete, otherwise from both.
fn join_handler_edge(
    try_exits: bool,
    try_edge_types: Vec<(String, Type)>,
    catch_edge_types: Vec<(String, Type)>,
    scopes: &mut ScopeStack,
) {
    if catch_edge_types.is_empty() {
        return;
    }
    if try_exits {
        adopt_branch_assignments(&catch_edge_types, scopes);
    } else {
        join_branch_edges(&[try_edge_types, catch_edge_types], scopes);
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
