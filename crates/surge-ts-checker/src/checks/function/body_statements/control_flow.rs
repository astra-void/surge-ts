
use surge_ts_diagnostics::{Diagnostic, DiagnosticCode};
use surge_ts_syntax::{
    ParsedBindingName, ParsedExpression, ParsedForOfStatement, ParsedIfStatement,
    ParsedSwitchStatement, ParsedTryStatement, ParsedType, ParsedWhileStatement,
};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    FlowCheck, FunctionFlowState, analyze_function_body_flow, check_expression_flow,
    merge_branch_deltas,
};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::ScopeStack;
use super::super::narrowing::narrow_reference_non_null_in_scope;
use super::super::{
    check_function_body, evaluate_condition_expression_with_truthy_guards, insert_binding_name,
    narrow_discriminant_in_scope, narrow_truthy_guarded_identifiers, visible_symbols,
};
use super::{
    adopt_branch_assignments, body_ends_in_never_call, branch_assigned_names,
    branch_assignment_types,
    join_branch_assignments, join_branch_pair, narrow_aliased_guard_after_exit,
    narrow_condition_and_aliases_in_scope, resolved_alias_condition, rewrite_discriminant_aliases,
};

pub(crate) fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
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
                flow_state,
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
                flow_state,
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
                flow_state,
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
                flow_state,
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
                flow_state,
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
                flow_state,
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
    let ParsedWhileStatement {
        condition,
        condition_span,
        body,
        runs_at_least_once,
    } = while_statement;

    // A lowered `do … while (c)` runs its body first, so the body is checked
    // with the enclosing flow state (its assignments stand afterwards) and the
    // condition is checked last, where it may read what the body assigned.
    if runs_at_least_once {
        scopes.push_child();
        check_function_body(body, return_type, scopes, flow_state, ctx);
        scopes.pop_child();
        check_while_condition(&condition, condition_span, statement_index, scopes, flow_state, ctx);
        return;
    }

    check_while_condition(&condition, condition_span, statement_index, scopes, flow_state, ctx);

    scopes.push_child();
    if flow_state.tracked_local_count() > 0 {
        flow_state.begin_branch_capture();
        check_function_body(body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
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
            element_type = Type::String;
            numeric_property_names = matches!(
                &iterable_type,
                InferredExpression::Known(iterable_type)
                    if has_numeric_property_names(iterable_type)
            );
        } else if let InferredExpression::Known(iterable_type) = iterable_type {
            element_type = for_of_element_type(&iterable_type);
        }
    }

    scopes.push_child();
    // `for (const _ in ref)` acts as a non-null assertion on `ref` for the
    // duration of the body (tsc: `getTypeAtFlowNode`, flow.go — "for (const _
    // in ref) acts as a nonnull on ref"), so indexing an optional object with
    // the key it just produced is not a possibly-undefined access.
    if for_of_statement.keys_only {
        narrow_reference_non_null_in_scope(&for_of_statement.iterable, scopes);
    }
    insert_binding_name(&for_of_statement.binding_name, element_type, scopes);
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
    if flow_active {
        flow_state.begin_branch_capture();
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
}

/// tsc's `hasNumericPropertyNames`: the type's only index signature is the
/// numeric one, which is what makes a `for…in` key over it a `number`.
fn has_numeric_property_names(ty: &Type) -> bool {
    match ty.peeled() {
        Type::Object(object) => object.has_numeric_property_names(),
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
pub(super) fn iterable_reference_element_type(reference: &surge_ts_types::TypeReference) -> Option<Type> {
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
            || matches!(case_type, Type::Any | Type::Undefined)
            || matches!(discriminant_type, Type::Any | Type::Undefined)
        {
            continue;
        }
        if crate::checks::ops::types_overlap_for_equality(case_type, discriminant_type) {
            continue;
        }
        let (case_name, discriminant_name) =
            crate::checks::ops::equality_operand_display_names(case_type, discriminant_type);
        let diagnostic = Diagnostic::ts2678(&case_name, &discriminant_name, ctx.file_name.clone());
        let diagnostic = match switch_case.test_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
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
    let no_case_matches =
        any_of(&mut switch_statement.cases.iter().filter_map(|case| case.test.as_ref()));
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
                            !group.iter().flatten().any(|member| std::ptr::eq(*member, *test))
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
    let has_default = switch_statement.cases.iter().any(|case| case.test.is_none());
    let every_case_exits = switch_statement.cases.last().is_some_and(|case| !case.consequent.is_empty())
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
            if let Some((condition, branch_is_true)) = case_group_conditions[case_index].as_ref() {
                narrow_discriminant_in_scope(condition, scopes, *branch_is_true, ctx);
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

    // Only the implicit `default` path continues past a `switch` whose every
    // case leaves it.
    if !has_default
        && every_case_exits
        && let Some(condition) = no_case_matches.as_ref()
    {
        narrow_discriminant_in_scope(condition, scopes, false, ctx);
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
        // Reaching the code after a `try` whose handler returns or throws means
        // the block completed, so its assignments hold from there on.
        let handler_diverts = try_statement.handler.as_ref().is_none_or(|handler| {
            let flow = analyze_function_body_flow(&handler.body);
            flow.guarantees_value_return || flow.guarantees_exit
        });
        let mut joinable_assignments = Vec::new();
        if handler_diverts && !try_guarantees_value_return {
            branch_assigned_names(&try_statement.block, &mut joinable_assignments);
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
        try_delta.continues = !try_guarantees_value_return;
        let try_assignment_types = branch_assignment_types(&joinable_assignments, scopes);
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
            catch_delta.continues = !catch_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(catch_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
        adopt_branch_assignments(&try_assignment_types, scopes);
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
