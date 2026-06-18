//! Function body and statement-level checking (control flow, returns, assignments).

use super::*;

use std::time::Instant;
use surge_ts_diagnostics::{Diagnostic, DiagnosticCode};
use surge_ts_syntax::{
    ParsedAssignment, ParsedBindingName, ParsedExpression, ParsedForOfStatement,
    ParsedFunctionBodyStatement, ParsedIfStatement, ParsedReturnStatement, ParsedSwitchStatement,
    ParsedThisPropertyAssignment, ParsedTryStatement, ParsedType, ParsedVariableDeclaration,
    ParsedVariableKind, ParsedWhileStatement,
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
    check_obvious_truthiness_condition, collect_future_block_scoped_declarations,
    mark_assignment_state, merge_branch_deltas,
};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::program::{
    record_flow_function_count, record_flow_function_skipped_count, record_flow_statement_count,
    record_function_body_check, record_program_timing,
};
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};

pub(crate) fn should_check_missing_return(return_type: &Type) -> bool {
    !matches!(
        return_type,
        Type::Any | Type::Unknown | Type::Undefined | Type::Void
    ) && !type_contains_unknown(return_type)
}

pub(crate) fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Array(element) => type_contains_unknown(element),
        Type::Tuple(elements) => elements.iter().any(type_contains_unknown),
        Type::Function(function) => {
            function.parameters().iter().any(type_contains_unknown)
                || type_contains_unknown(function.return_type())
        }
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_contains_unknown(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(type_contains_unknown)
        }
        Type::Union(union) => union.types().iter().any(type_contains_unknown),
        _ => false,
    }
}

pub(crate) fn emit_missing_return_diagnostic(
    body_flow: crate::flow::FunctionBodyFlow,
    missing_return_span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) {
    let with_span = |diagnostic: Diagnostic| match missing_return_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };

    if body_flow.contains_value_return {
        if !body_flow.guarantees_value_return {
            ctx.push(with_span(Diagnostic::ts2366(ctx.file_name.clone())));
        }
    } else {
        ctx.push(with_span(Diagnostic::ts2355(ctx.file_name.clone())));
    }
}

pub(crate) fn check_function_body(
    body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    record_function_body_check();
    record_flow_function_count();

    let mut pushed_scope = false;
    if flow_state.is_enabled() {
        let future_block_scoped_declarations = collect_future_block_scoped_declarations(&body);
        if !future_block_scoped_declarations.is_empty() {
            flow_state.push_scope(future_block_scoped_declarations);
            pushed_scope = true;
        }
    } else {
        record_flow_function_skipped_count();
    }

    for (statement_index, statement) in body.into_iter().enumerate() {
        check_function_body_statement(
            statement,
            statement_index,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
    }

    if pushed_scope {
        flow_state.pop_scope();
    }
}

pub(crate) fn check_function_body_statement(
    statement: ParsedFunctionBodyStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    record_flow_statement_count();
    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            check_function_variable_declaration(variable, statement_index, scopes, flow_state, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.variable_declaration_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Block(block_body) => {
            check_function_block(block_body, return_type, scopes, flow_state, ctx);
        }
        ParsedFunctionBodyStatement::Return(return_statement) => {
            let start = Instant::now();
            let visible_symbols = visible_symbols(scopes);
            check_function_return_statement(
                return_statement,
                statement_index,
                return_type,
                flow_state,
                &visible_symbols,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.return_statement_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Throw(throw_statement) => {
            check_function_throw_statement(
                throw_statement,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::Assignment(assignment) => {
            let start = Instant::now();
            check_function_assignment(assignment, statement_index, scopes, flow_state, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::ThisPropertyAssignment(assignment) => {
            let start = Instant::now();
            check_this_property_assignment(assignment, scopes, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            let start = Instant::now();
            check_function_expression_statement(
                expression,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.expression_statement_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            let start = Instant::now();
            check_function_if_statement(
                if_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            let start = Instant::now();
            check_function_while_statement(
                while_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
            let start = Instant::now();
            check_function_for_of_statement(
                for_of_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            let start = Instant::now();
            check_function_switch_statement(
                switch_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            let start = Instant::now();
            check_function_try_statement(
                try_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
    }
}

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
        scopes.insert_current_handle(local_name.as_str(), symbol);
        apply_variable_declaration_state(variable_kind, local_name, has_initializer, flow_state);
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

pub(crate) fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(&if_statement.condition, if_statement.condition_span, ctx);

    let then_guarantees_value_return =
        analyze_function_body_flow(&if_statement.then_body).guarantees_value_return;
    let has_else_body = !if_statement.else_body.is_empty();

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
        narrow_discriminant_in_scope(&if_statement.condition, scopes, true);
        flow_state.begin_branch_capture();
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let mut then_delta = flow_state.finish_branch_capture();
        then_delta.continues = !then_guarantees_value_return;
        scopes.pop_child();
        branch_deltas.push(then_delta);

        if has_else_body {
            let else_guarantees_value_return =
                analyze_function_body_flow(&if_statement.else_body).guarantees_value_return;
            scopes.push_child();
            narrow_discriminant_in_scope(&if_statement.condition, scopes, false);
            flow_state.begin_branch_capture();
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            let mut else_delta = flow_state.finish_branch_capture();
            else_delta.continues = !else_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(else_delta);
        }

        if !has_else_body && then_guarantees_value_return {
            narrow_truthy_guarded_identifiers(&if_statement.condition, scopes);
            narrow_discriminant_in_scope(&if_statement.condition, scopes, false);
        }

        merge_branch_deltas(flow_state, &branch_deltas, !has_else_body);
    } else {
        scopes.push_child();
        narrow_discriminant_in_scope(&if_statement.condition, scopes, true);
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();

        if has_else_body {
            scopes.push_child();
            narrow_discriminant_in_scope(&if_statement.condition, scopes, false);
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            scopes.pop_child();
        }

        if !has_else_body && then_guarantees_value_return {
            narrow_truthy_guarded_identifiers(&if_statement.condition, scopes);
            narrow_discriminant_in_scope(&if_statement.condition, scopes, false);
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
        _ => Type::Unknown,
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

    if flow_active {
        let mut branch_deltas = Vec::new();

        for switch_case in switch_statement.cases {
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
        for switch_case in switch_statement.cases {
            scopes.push_child();
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
                    if !matches!(declared_type, ParsedType::Any | ParsedType::Unknown) {
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
                    if !matches!(declared_type, ParsedType::Any | ParsedType::Unknown) {
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

    if value_type == Type::Unknown || property_type == Type::Unknown {
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

    if value_ty == Type::Unknown {
        return;
    }

    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };

    let updated_ty = if symbol.ty == Type::Undefined {
        union_type(vec![
            Type::Undefined,
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || value_ty.clone()),
        ])
    } else if symbol.ty == value_ty || is_assignable_to(&value_ty, &symbol.ty) {
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    } else if matches!(symbol.ty, Type::Any | Type::Unknown) {
        union_type(vec![
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
            value_ty,
        ])
    } else {
        // Preserve the declared/inferred symbol type when an incompatible assignment
        // is already reported to avoid cascading return/usage diagnostics.
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    };

    if updated_ty == symbol.ty {
        return;
    }

    let _ = scopes.update_visible(
        target_name,
        SymbolInfo {
            ty: updated_ty,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
    );
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
        return;
    };

    let inferred_expression = evaluate_expression_with_expected_type(
        expression,
        return_statement.expression_span,
        Some(return_type),
        ExpectedTypeDiagnostic::TypeNotAssignable,
        symbols,
        ctx,
    );

    match inferred_expression {
        InferredExpression::Known(source_type) => {
            if source_type == Type::Unknown {
                return;
            }

            if !is_assignable_to(&source_type, &return_type) {
                let source_type_name =
                    crate::checks::expr::source_display_name(&source_type, &return_type);
                let target_type_name = return_type.name();
                let diagnostic =
                    Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

                let diagnostic = match return_statement.expression_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };

                ctx.push(diagnostic);
            }
        }
        InferredExpression::UnresolvedIdentifier { .. } => {}
        InferredExpression::MissingProperty { .. } => {}
        InferredExpression::Unknown => {}
    }
}

pub(crate) fn visible_symbols(scopes: &ScopeStack) -> &SymbolTable {
    scopes.visible_symbols()
}

pub(crate) fn check_local_duplicate_declaration(
    variable: &ParsedVariableDeclaration,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    if matches!(
        variable.kind,
        ParsedVariableKind::Let | ParsedVariableKind::Const
    ) && scopes.current_contains_let_or_const(&variable.name)
    {
        let diagnostic = Diagnostic::ts2451(&variable.name, ctx.file_name.clone());
        let diagnostic = match variable.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
    }
}
