//! Function body and statement-level checking (control flow, returns, assignments).

use super::*;

use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedFunctionBodyStatement, ParsedVariableDeclaration, ParsedVariableKind};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{FunctionFlowState, collect_future_block_scoped_declarations};
use crate::program::{
    record_flow_function_count, record_flow_function_skipped_count, record_flow_statement_count,
    record_function_body_check, record_program_timing,
};
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};

pub(crate) fn should_check_missing_return(return_type: &Type) -> bool {
    !matches!(
        return_type,
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::Undefined | Type::Void
    ) && !type_contains_unknown(return_type)
}

pub(crate) fn type_contains_unknown(ty: &Type) -> bool {
    thread_local! {
        // References resolved while walking the current type, to break the cyclic
        // structural graphs lazy nominal references form (interface A whose member
        // resolves to B whose member resolves back to A). Re-entering a reference
        // already on this path means the cycle introduces no *new* `unknown`.
        static VISITING_REFERENCES: std::cell::RefCell<Vec<(std::sync::Arc<str>, std::sync::Arc<[Type]>)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    match ty {
        Type::Unknown | Type::GenuineUnknown => true,
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
        Type::Reference(reference) => {
            let on_path = VISITING_REFERENCES.with(|visiting| {
                visiting
                    .borrow()
                    .iter()
                    .any(|(id, arguments)| *id == reference.id && *arguments == reference.arguments)
            });
            if on_path {
                return false;
            }
            VISITING_REFERENCES.with(|visiting| {
                visiting
                    .borrow_mut()
                    .push((reference.id.clone(), reference.arguments.clone()));
            });
            let result = type_contains_unknown(&reference.resolve());
            VISITING_REFERENCES.with(|visiting| {
                visiting.borrow_mut().pop();
            });
            result
        }
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

/// TS7030 under `noImplicitReturns`: an un-annotated function where some path
/// returns a value but the end point is still reachable. The annotated analogue
/// is [`emit_missing_return_diagnostic`]'s TS2366 branch.
pub(crate) fn emit_implicit_return_diagnostic(
    missing_return_span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) {
    let diagnostic = Diagnostic::ts7030(ctx.file_name.clone());
    let diagnostic = match missing_return_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };
    ctx.push(diagnostic);
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

    // Hoist nested `function` declarations into the current scope so a sibling
    // closure can call them (function declarations are function-scoped and
    // callable before their statement position).
    for statement in &body {
        if let ParsedFunctionBodyStatement::Function(function) = statement {
            let function_type = crate::checks::function::signature::map_function_signature(
                &function.parameters,
                function.return_type.as_ref(),
                &function.type_parameters,
                None,
                ctx,
            );
            scopes.insert_current_handle(
                function.name.as_str(),
                std::sync::Arc::new(SymbolInfo {
                    ty: Type::Function(function_type),
                    kind: crate::symbols::SymbolKind::Function,
                    function_signature: None,
                }),
            );
        }
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
        // A nested function declaration is inert for the enclosing body's
        // checking (its body is not separately type-checked, matching the prior
        // drop-at-parse behavior); it is retained only so use-tracking can see
        // identifier reads inside it.
        ParsedFunctionBodyStatement::Function(_) => {}
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            check_function_variable_declaration(
                *variable,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
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
                *return_statement,
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
                *throw_statement,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::Continue | ParsedFunctionBodyStatement::Break => {}
        ParsedFunctionBodyStatement::Assignment(assignment) => {
            let start = Instant::now();
            check_function_assignment(*assignment, statement_index, scopes, flow_state, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::ThisPropertyAssignment(assignment) => {
            let start = Instant::now();
            check_this_property_assignment(*assignment, scopes, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            let start = Instant::now();
            check_function_expression_statement(
                *expression,
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
                *if_statement,
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
                *while_statement,
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
                *for_of_statement,
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
                *switch_statement,
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
                *try_statement,
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
