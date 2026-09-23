//! Per-statement program checking and unsupported-declaration diagnostics.

use crate::checks::function::body_statements::evolving_arrays;
use std::collections::HashMap;
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedFunctionDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan,
};
use surge_ts_types::FunctionType;

use super::*;

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::CheckerContext;

pub(crate) fn check_program_file_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    let classes = super::forward_references::file_class_declarations(statements);
    check_overload_implementation_compatibility(statements, file_index, function_signatures, ctx);
    for (statement_index, statement) in statements.iter().cloned().enumerate() {
        let statement = expand_module_if_alias(statement, &statements[..statement_index]);
        super::forward_references::check_statement_forward_references(&statement, &classes, ctx);
        check_program_statement(
            statement,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        );
    }
    crate::flow::check_module_definite_assignment(statements, ctx);
}

/// tsc's `isImplementationCompatibleWithOverload` for a module-level function
/// overload group (TS2394 on each overload): the return types must be related
/// in either direction unless the overload returns `void`, the implementation
/// may not require more arguments than the overload declares, and each of the
/// overload's parameters must be assignable to the implementation's (strict
/// variance, as for any function declaration). A generic group is erased to
/// `any` by tsc, which surge approximates by not checking it; an implementation
/// without a written return type, or any type surge could not resolve, is
/// likewise left alone.
fn check_overload_implementation_compatibility(
    statements: &[ParsedStatement],
    file_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    fn function_at(statement: &ParsedStatement) -> Option<&ParsedFunctionDeclaration> {
        match statement {
            ParsedStatement::FunctionDeclaration(function) => Some(function),
            ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
                ParsedExportDeclaration::Statement { declaration, .. } => match declaration.as_ref() {
                    ParsedStatement::FunctionDeclaration(function) => Some(function),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
    let mut index = 0;
    while index < statements.len() {
        let Some(first) = function_at(&statements[index]) else {
            index += 1;
            continue;
        };
        let mut end = index + 1;
        while end < statements.len()
            && function_at(&statements[end]).is_some_and(|function| function.name == first.name)
        {
            end += 1;
        }
        let group: Vec<(usize, &ParsedFunctionDeclaration)> = (index..end)
            .filter_map(|position| function_at(&statements[position]).map(|function| (position, function)))
            .collect();
        index = end;
        let Some(&(implementation_index, implementation)) = group.last() else {
            continue;
        };
        if group.len() < 2
            || !implementation.has_body
            || implementation.is_declare
            || group.iter().any(|(_, function)| !function.type_parameters.is_empty())
        {
            continue;
        }
        let signature_at = |position: usize| {
            function_signatures.get(&FunctionDeclarationLocation {
                file_index,
                statement_index: position,
            })
        };
        let Some(implementation_type) = signature_at(implementation_index) else {
            continue;
        };
        for &(position, overload) in &group[..group.len() - 1] {
            if overload.has_body {
                continue;
            }
            let Some(overload_type) = signature_at(position) else {
                continue;
            };
            if overload_is_compatible(
                implementation_type,
                implementation.return_type.is_some(),
                overload_type,
            ) {
                continue;
            }
            ctx.push(crate::spans::diagnostic_with_syntax_span(
                Diagnostic::ts2394(ctx.file_name.clone()),
                overload.name_span,
            ));
        }
    }
}

fn overload_is_compatible(
    implementation: &FunctionType,
    implementation_returns_written: bool,
    overload: &FunctionType,
) -> bool {
    use surge_ts_types::{Type, is_assignable_to};
    let unresolved = |ty: &Type| ty.is_unknown() || matches!(ty, Type::TypeParameter(_) | Type::ErrorType);
    let related = |left: &Type, right: &Type| {
        unresolved(left) || unresolved(right) || is_assignable_to(left, right) || is_assignable_to(right, left)
    };
    if implementation_returns_written
        && !matches!(overload.return_type(), Type::Void)
        && !related(implementation.return_type(), overload.return_type())
    {
        return false;
    }
    // A tuple rest spreads into positions surge's tuples cannot mark optional.
    let non_array_rest = |signature: &FunctionType| {
        signature.is_variadic()
            && signature
                .parameters()
                .last()
                .is_some_and(|rest| !matches!(rest.peeled(), Type::Array(_)))
    };
    if non_array_rest(implementation) || non_array_rest(overload) {
        return true;
    }
    let overload_count = overload.parameters().len();
    if !overload.is_variadic() && implementation.required_parameter_count() > overload_count {
        return false;
    }
    let parameter_at = |signature: &FunctionType, position: usize| -> Option<Type> {
        let parameters = signature.parameters();
        if signature.is_variadic() && position + 1 >= parameters.len() {
            return parameters.last().map(|rest| match rest.peeled() {
                Type::Array(element) => *element,
                other => other,
            });
        }
        let parameter = parameters.get(position)?;
        Some(if position >= signature.required_parameter_count() {
            surge_ts_types::union_type(vec![parameter.clone(), Type::Undefined])
        } else {
            parameter.clone()
        })
    };
    let count = implementation.parameters().len().max(overload_count);
    (0..count).all(|position| {
        match (parameter_at(implementation, position), parameter_at(overload, position)) {
            (Some(source), Some(target)) => {
                unresolved(&source) || unresolved(&target) || is_assignable_to(&target, &source)
            }
            _ => true,
        }
    })
}

/// `const ok = typeof v === "string"; if (ok) …` narrows by the condition the
/// alias was written as, as a function body does (tsc's aliased-condition
/// narrowing). Module scope keeps no flow state to record the alias in, so the
/// `const` is looked up among the statements before the `if`.
pub(crate) fn expand_module_if_alias(
    statement: ParsedStatement,
    preceding: &[ParsedStatement],
) -> ParsedStatement {
    let ParsedStatement::If(mut if_statement) = statement else {
        return statement;
    };
    if let Some(expanded) = expand_module_alias_condition(&if_statement.condition, preceding) {
        if_statement.condition = expanded;
    }
    ParsedStatement::If(if_statement)
}

fn expand_module_alias_condition(
    condition: &surge_ts_syntax::ParsedExpression,
    preceding: &[ParsedStatement],
) -> Option<surge_ts_syntax::ParsedExpression> {
    use surge_ts_syntax::ParsedExpression;
    match condition {
        ParsedExpression::Identifier { name, .. } => module_alias_condition(name, preceding),
        ParsedExpression::Unary {
            operator: operator @ surge_ts_syntax::ParsedUnaryOperator::Not,
            operator_span,
            operand,
            operand_span,
        } => Some(ParsedExpression::Unary {
            operator: *operator,
            operator_span: *operator_span,
            operand: Box::new(expand_module_alias_condition(operand, preceding)?),
            operand_span: *operand_span,
        }),
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let expanded_left = expand_module_alias_condition(left, preceding);
            let expanded_right = expand_module_alias_condition(right, preceding);
            if expanded_left.is_none() && expanded_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Logical {
                left: Box::new(expanded_left.unwrap_or_else(|| (**left).clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(expanded_right.unwrap_or_else(|| (**right).clone())),
                right_span: *right_span,
            })
        }
        _ => None,
    }
}

/// The condition the latest module declaration of `name` aliases, when it is a
/// local, unannotated `const` initialized with one. An exported one resolves to
/// its export symbol, which tsc does not inline.
fn module_alias_condition(
    name: &str,
    preceding: &[ParsedStatement],
) -> Option<surge_ts_syntax::ParsedExpression> {
    let variable = preceding.iter().rev().find_map(|statement| match statement {
        ParsedStatement::VariableDeclaration(variable) if variable.name == name => {
            Some(Some(variable))
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => match declaration.as_ref() {
                ParsedStatement::VariableDeclaration(variable) if variable.name == name => {
                    Some(None)
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    })??;
    if !matches!(variable.kind, surge_ts_syntax::ParsedVariableKind::Const)
        || variable.is_declare
        || variable.declared_type.is_some()
    {
        return None;
    }
    variable
        .initializer
        .as_ref()
        .filter(|initializer| crate::checks::function::is_condition_shaped(initializer))
        .cloned()
}

/// Runs `check` with every narrowed module `let`/`var` read at its declared type.
/// A function declaration is hoisted and may run before the narrowing
/// assignment or guard, so its body does not see module narrowing of a binding
/// that can still change.
pub(crate) fn with_declared_mutable_module_bindings(
    ctx: &mut CheckerContext,
    check: impl FnOnce(&mut CheckerContext),
) {
    let narrowed: Vec<(std::sync::Arc<str>, crate::symbols::SymbolInfo, surge_ts_types::Type)> = ctx
        .symbols
        .narrowed_names()
        .filter_map(|name| {
            let symbol = ctx.symbols.get(name)?;
            if !matches!(
                symbol.kind,
                crate::symbols::SymbolKind::Let | crate::symbols::SymbolKind::Var
            ) {
                return None;
            }
            let declared = ctx.symbols.declared_type(name)?;
            (*declared != symbol.ty)
                .then(|| (std::sync::Arc::clone(name), symbol.clone(), declared.clone()))
        })
        .collect();
    // Widened in place, not on a copy: the check records declaration spans on
    // the module table, which must survive it.
    for (name, symbol, declared) in &narrowed {
        let widened = crate::symbols::SymbolInfo {
            ty: declared.clone(),
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        let _ = ctx.symbols.insert(std::sync::Arc::clone(name), widened);
    }
    check(ctx);
    for (name, symbol, declared) in narrowed {
        let _ = ctx.symbols.insert_narrowed(name, symbol, declared);
    }
}

/// A module-scope assignment narrows the binding for the statements after it, as
/// the same assignment does in a block (`x ??= v` included, which lowers to
/// `x = x ?? v`). The assignment check itself is unchanged; the value's type is
/// read again without its diagnostics to decide the narrowing.
pub(crate) fn check_module_assignment(
    assignment: surge_ts_syntax::ParsedAssignment,
    ctx: &mut CheckerContext,
) {
    let target_name = assignment.target_name.clone();
    let value = assignment.value.clone();
    let value_span = assignment.value_span;
    assign::check_assignment(assignment, ctx);

    let Some(original) = ctx.symbols.get(&target_name) else {
        return;
    };
    if matches!(original.kind, crate::symbols::SymbolKind::Const) {
        return;
    }
    let original_ty = original.ty.clone();
    let symbols = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    let checkpoint = ctx.diagnostics().len();
    let inferred = expr::evaluate_expression(&value, value_span, &symbols, ctx);
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);

    if evolving_arrays::assign_module_evolving_array(&target_name, &value, &inferred, ctx) {
        return;
    }

    let mut scopes = crate::symbols::ScopeStack::from_root(symbols);
    check_function::update_assigned_symbol_type(&target_name, inferred, &mut scopes);
    let Some(updated) = scopes.resolve(&target_name) else {
        return;
    };
    if updated.ty == original_ty {
        return;
    }
    let updated = updated.clone();
    let declared = ctx
        .symbols
        .declared_type(&target_name)
        .cloned()
        .unwrap_or(original_ty);
    let _ = ctx.symbols.insert_narrowed(target_name, updated, declared);
}

/// A module-scope call written as a statement, as the expression an assertion
/// signature is read from, when it could narrow an argument.
pub(crate) fn module_call_expression(
    call: &surge_ts_syntax::ParsedCall,
) -> Option<surge_ts_syntax::ParsedExpression> {
    assertion_arguments_named(&call.arguments).then(|| surge_ts_syntax::ParsedExpression::Call {
        callee_name: call.callee_name.clone(),
        callee_span: call.callee_span,
        type_arguments: call.type_arguments.clone(),
        arguments: call.arguments.clone(),
    })
}

pub(crate) fn assertion_candidate(expression: &surge_ts_syntax::ParsedExpression) -> bool {
    match expression {
        surge_ts_syntax::ParsedExpression::Call { arguments, .. }
        | surge_ts_syntax::ParsedExpression::PropertyCall { arguments, .. } => {
            assertion_arguments_named(arguments)
        }
        _ => false,
    }
}

fn assertion_arguments_named(arguments: &[surge_ts_syntax::ParsedCallArgument]) -> bool {
    arguments.iter().any(|argument| {
        matches!(argument.expression, surge_ts_syntax::ParsedExpression::Identifier { .. })
    })
}

/// `assertIsString(value);` narrows `value` for the rest of the module, as it
/// does for the rest of a block (tsc's assertion signatures). The narrowing runs
/// over a scope rooted at the module's symbols, and each argument binding it
/// changed is carried back.
pub(crate) fn narrow_module_assertion_call(
    expression: Option<surge_ts_syntax::ParsedExpression>,
    ctx: &mut CheckerContext,
) {
    let Some(expression) = expression else {
        return;
    };
    let arguments = match &expression {
        surge_ts_syntax::ParsedExpression::Call { arguments, .. }
        | surge_ts_syntax::ParsedExpression::PropertyCall { arguments, .. } => arguments,
        _ => return,
    };
    let names: Vec<String> = arguments
        .iter()
        .filter_map(|argument| match &argument.expression {
            surge_ts_syntax::ParsedExpression::Identifier { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    let mut scopes = crate::symbols::ScopeStack::from_root(
        ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext),
    );
    crate::checks::function::narrow_assertion_call_in_scope(&expression, &mut scopes, ctx);
    for name in names {
        let (Some(narrowed), Some(original)) = (scopes.resolve(&name), ctx.symbols.get(&name))
        else {
            continue;
        };
        if narrowed.ty == original.ty {
            continue;
        }
        let narrowed = narrowed.clone();
        let declared = original.ty.clone();
        let _ = ctx.symbols.insert_narrowed(name, narrowed, declared);
    }
}

/// A module-scope `if`: each branch is checked under its own narrowing, and
/// when one branch cannot fall through (`if (isCancel(value)) process.exit(0)`)
/// the statements after it see the other branch's narrowing, exactly as a
/// function body would.
pub(crate) fn check_module_if_statement(
    if_statement: &surge_ts_syntax::ParsedIfStatement,
    ctx: &mut CheckerContext,
) {
    let symbols = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    crate::checks::function::report_unreferenced_callable_conditions(
        &if_statement.unreferenced_truthiness_tests,
        &symbols,
        ctx,
    );
    let condition = crate::checks::expr::evaluate_expression(
        &if_statement.condition,
        if_statement.condition_span,
        &symbols,
        ctx,
    );
    crate::checks::expr::report_void_truthiness(&condition, if_statement.condition_span, ctx);
    let mut assigned = Vec::new();
    crate::checks::function::branch_assigned_names(&if_statement.then_body, &mut assigned);
    crate::checks::function::branch_assigned_names(&if_statement.else_body, &mut assigned);
    let mut branch_end_types = Vec::new();
    for (branch, branch_is_true) in [
        (&if_statement.then_body, true),
        (&if_statement.else_body, false),
    ] {
        let branch_symbols = narrowed_module_symbols(&if_statement.condition, branch_is_true, ctx)
            .unwrap_or_else(|| {
                ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext)
            });
        let end_types = if branch.is_empty() {
            assigned
                .iter()
                .map(|name| branch_symbols.get(name).map(|symbol| symbol.ty.clone()))
                .collect()
        } else {
            check_statements_over_module_scope(branch.clone(), branch_symbols, &assigned, ctx)
        };
        branch_end_types.push(end_types);
    }
    let scopes = crate::symbols::ScopeStack::from_root(symbols);
    let diverts = |body: &[surge_ts_syntax::ParsedFunctionBodyStatement]| {
        let flow = crate::flow::analyze_function_body_flow(body);
        flow.guarantees_value_return
            || flow.guarantees_exit
            || crate::checks::function::body_ends_in_never_call(body, &scopes)
    };
    let then_diverts = diverts(&if_statement.then_body);
    let else_diverts = !if_statement.else_body.is_empty() && diverts(&if_statement.else_body);
    if !then_diverts && !else_diverts {
        join_module_branch_assignments(&assigned, &branch_end_types, ctx);
    }
    let surviving_branch = match (then_diverts, else_diverts) {
        (true, false) => false,
        (false, true) => true,
        _ => return,
    };
    if let Some(narrowed) =
        narrowed_module_symbols(&if_statement.condition, surviving_branch, ctx)
    {
        ctx.symbols = narrowed;
    }
}

/// The module symbols as narrowed by `condition` holding (or not), when the
/// condition narrows anything.
fn narrowed_module_symbols(
    condition: &surge_ts_syntax::ParsedExpression,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> Option<crate::symbols::SymbolTable> {
    let base = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    let narrowed =
        crate::checks::function::narrow_condition_symbol_table(condition, &base, branch_is_true);
    let narrowed = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        narrowed.as_ref().unwrap_or(&base),
        branch_is_true,
        ctx,
    )
    .or(narrowed);
    // As in a function body, a guarded `unknown` stops reading as the `unknown`
    // keyword in the branch where its guard holds.
    crate::checks::expr::downgrade_guarded_genuine_unknown(
        condition,
        narrowed.as_ref().unwrap_or(&base),
        branch_is_true,
    )
    .or(narrowed)
}

/// A module-scope loop, block, `switch` or `try` is checked exactly as the
/// same statement in a function body, over a scope rooted at the module's
/// symbols.
pub(crate) fn check_module_block(
    statements: Vec<surge_ts_syntax::ParsedFunctionBodyStatement>,
    ctx: &mut CheckerContext,
) {
    let symbols = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    let _ = check_statements_over_module_scope(statements, symbols, &[], ctx);
}

/// Checks `statements` over a scope rooted at `symbols` and answers the type
/// each of `names` ends with.
fn check_statements_over_module_scope(
    statements: Vec<surge_ts_syntax::ParsedFunctionBodyStatement>,
    symbols: crate::symbols::SymbolTable,
    names: &[String],
    ctx: &mut CheckerContext,
) -> Vec<Option<surge_ts_types::Type>> {
    let mut scopes = crate::symbols::ScopeStack::from_root(symbols);
    // The block is a scope of its own: a `const name` in it shadows the module
    // binding or the global of that name instead of redeclaring it. The frame
    // stays pushed so `names` still read what the block narrowed them to.
    scopes.push_child();
    let flow_facts = crate::flow::collect_function_flow_facts(&statements);
    let mut flow_state = crate::flow::FunctionFlowState::new(
        flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
    );
    let mut var_names = Vec::new();
    crate::flow::collect_var_names(&statements, &mut var_names);
    crate::checks::function::check_function_body(
        statements,
        None,
        &mut scopes,
        &mut flow_state,
        ctx,
    );
    // A `var` is function-scoped, so one declared in a module-level block,
    // branch or loop is a module binding once the statement has run.
    for name in var_names {
        if let Some(symbol) = scopes.visible_symbols().get(&name)
            && matches!(symbol.kind, crate::symbols::SymbolKind::Var)
            && ctx
                .symbols
                .get(&name)
                .is_none_or(|existing| matches!(existing.kind, crate::symbols::SymbolKind::Var))
        {
            let symbol = symbol.clone();
            let _ = ctx.symbols.insert(name, symbol);
        }
    }
    names
        .iter()
        .map(|name| scopes.resolve(name).map(|symbol| symbol.ty.clone()))
        .collect()
}

/// Both edges of a module-scope `if` reach the code after it, so a binding
/// either branch assigned is the union of what each edge left it as — bounded,
/// as in a function body, by its declaration.
fn join_module_branch_assignments(
    names: &[String],
    branch_end_types: &[Vec<Option<surge_ts_types::Type>>],
    ctx: &mut CheckerContext,
) {
    for (index, name) in names.iter().enumerate() {
        let Some(edges) = branch_end_types
            .iter()
            .map(|types| types.get(index).cloned().flatten())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some(current) = ctx.symbols.get(name) else {
            continue;
        };
        let joined = surge_ts_types::union_type(edges);
        let current = current.clone();
        let declared = ctx.symbols.declared_type(name).cloned().unwrap_or(current.ty.clone());
        if joined == current.ty || !surge_ts_types::is_assignable_to(&joined, &declared) {
            continue;
        }
        let narrowed = crate::symbols::SymbolInfo {
            ty: joined,
            kind: current.kind,
            function_signature: current.function_signature.clone(),
        };
        let _ = ctx.symbols.insert_narrowed(name.clone(), narrowed, declared);
    }
}

fn with_module_export(ctx: &mut CheckerContext, check: impl FnOnce(&mut CheckerContext)) {
    ctx.module_export_depth += 1;
    check(ctx);
    ctx.module_export_depth -= 1;
}

fn with_module_declared_only(ctx: &mut CheckerContext, check: impl FnOnce(&mut CheckerContext)) {
    ctx.module_declared_only_depth += 1;
    check(ctx);
    ctx.module_declared_only_depth -= 1;
}

/// A module-level `[]` binding tsc types as an evolving array (`autoArrayType`)
/// under `noImplicitAny`. An exported one is not flow-typed: other modules can
/// mutate it, so tsc keeps its declared type.
fn module_auto_array(
    variable: &surge_ts_syntax::ParsedVariableDeclaration,
    ctx: &CheckerContext,
) -> Option<(std::sync::Arc<str>, crate::symbols::AutoArrayBinding)> {
    let name_span = variable.name_span?;
    if !var::is_auto_array_candidate(variable, ctx) || ctx.module_export_depth > 0 {
        return None;
    }
    let is_let = matches!(variable.kind, surge_ts_syntax::ParsedVariableKind::Let);
    Some((
        variable.name.as_str().into(),
        crate::symbols::AutoArrayBinding {
            name_span: Some(name_span),
            is_const: matches!(variable.kind, surge_ts_syntax::ParsedVariableKind::Const),
            declared_array: true,
            is_let,
            initialized: true,
            assignments: is_let
                .then(|| ctx.let_assignment(name_span.start))
                .flatten(),
            evolving: true,
            elements: surge_ts_types::Type::Never,
            unsettled: false,
            pending_loops: 0,
            declared_only: false,
            module_level: true,
        },
    ))
}

pub(crate) fn check_program_statement(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    // Collected before the statement is consumed and applied once it is
    // checked, where tsc's flow places an array mutation.
    let mutations = if ctx.auto_arrays_declared && ctx.symbols.has_auto_arrays() {
        evolving_arrays::collect_module_mutations(&statement, &ctx.symbols)
    } else {
        Vec::new()
    };
    check_program_statement_itself(
        statement,
        file_index,
        statement_index,
        function_signatures,
        ctx,
    );
    if !mutations.is_empty() {
        evolving_arrays::apply_module_mutations(mutations, ctx);
    }
}

fn check_program_statement_itself(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            let auto_array = module_auto_array(&variable, ctx);
            var::check_variable_declaration(*variable, ctx);
            if let Some((name, binding)) = auto_array {
                ctx.auto_arrays_declared = true;
                ctx.symbols.set_auto_array(name, Some(binding));
            }
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.variable_declaration_checking += start.elapsed()
            });
        }
        ParsedStatement::Assignment(assignment) => {
            check_module_assignment(*assignment, ctx);
        }
        ParsedStatement::MemberAssignment(assignment) => {
            let symbols = ctx
                .symbols
                .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
            let mut scopes = crate::symbols::ScopeStack::from_root(symbols);
            let receiver = match &assignment.target {
                surge_ts_syntax::ParsedExpression::PropertyAccess {
                    object,
                    property_name,
                    ..
                } => match object.as_ref() {
                    surge_ts_syntax::ParsedExpression::Identifier { name, .. } => {
                        Some((name.clone(), property_name.clone()))
                    }
                    _ => None,
                },
                _ => None,
            };
            crate::checks::function::check_member_assignment(*assignment, &mut scopes, ctx);
            // The scope stack is this statement's alone. An expando member the
            // write *declared* (`fn.x = v`) belongs to the binding, so it is
            // carried back for the statements that follow; what a write to an
            // existing member narrowed is not.
            if let Some((name, property_name)) = receiver
                && let Some(updated) = scopes.resolve(&name)
                && let surge_ts_types::Type::Object(object) = &updated.ty
                && object.call_signature().is_some()
                && ctx.symbols.get(&name).is_some_and(|current| {
                    current.ty.get_property_access_type(&property_name).is_none()
                })
            {
                let updated = crate::symbols::SymbolInfo {
                    ty: updated.ty.clone(),
                    kind: updated.kind,
                    function_signature: updated.function_signature.clone(),
                };
                let _ = ctx.symbols.insert(name, updated);
            }
        }
        ParsedStatement::FunctionDeclaration(function) => {
            with_module_declared_only(ctx, |ctx| {
                with_declared_mutable_module_bindings(ctx, |ctx| {
                    check_program_function_declaration(
                        *function,
                        file_index,
                        statement_index,
                        function_signatures,
                        ctx,
                    );
                });
            });
        }
        ParsedStatement::Call(call) => {
            let assertion = module_call_expression(&call);
            call::check_call(*call, ctx);
            narrow_module_assertion_call(assertion, ctx);
        }
        ParsedStatement::Expression(expression) => {
            let assertion = assertion_candidate(&expression).then(|| (*expression).clone());
            expr::check_expression_statement(*expression, ctx);
            narrow_module_assertion_call(assertion, ctx);
        }
        ParsedStatement::If(if_statement) => check_module_if_statement(&if_statement, ctx),
        ParsedStatement::Block(statements) => check_module_block(statements, ctx),
        ParsedStatement::TypeAliasDeclaration(alias) => {
            crate::checks::function::check_type_parameter_declarations(&alias.type_parameters, ctx);
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            crate::checks::function::check_type_parameter_declarations(
                &interface.type_parameters,
                ctx,
            );
            super::heritage::check_interface_heritage(&interface, ctx);
            super::index_constraints::check_interface_index_constraints(&interface, ctx);
        }
        ParsedStatement::ClassDeclaration(class) => {
            with_module_declared_only(ctx, |ctx| super::check_class_declaration(&class, ctx));
        }
        ParsedStatement::ImportDeclaration(_) => {}
        ParsedStatement::ExportDeclaration(export) => match *export {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                with_module_export(ctx, |ctx| {
                    check_program_statement(
                        *declaration,
                        file_index,
                        statement_index,
                        function_signatures,
                        ctx,
                    );
                });
            }
            ParsedExportDeclaration::Named { .. } => {}
            ParsedExportDeclaration::Namespace { .. } => {}
            ParsedExportDeclaration::Default { declaration, .. } => match declaration {
                ParsedDefaultExportDeclaration::Function(function) => {
                    with_module_declared_only(ctx, |ctx| {
                        check_program_function_declaration(
                            function,
                            file_index,
                            statement_index,
                            function_signatures,
                            ctx,
                        );
                    });
                }
                ParsedDefaultExportDeclaration::Expression(expression) => {
                    check_export_assignment_expression(expression, ctx);
                }
                ParsedDefaultExportDeclaration::Class(class) => {
                    with_module_declared_only(ctx, |ctx| {
                        super::check_class_declaration(&class, ctx);
                    });
                }
                ParsedDefaultExportDeclaration::Unsupported { span } => {
                    let mut diagnostic =
                        Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                    if let Some(span) = span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }

                    ctx.push(diagnostic);
                }
            },
            ParsedExportDeclaration::All { .. } => {}
            ParsedExportDeclaration::Empty { .. } => {}
            ParsedExportDeclaration::Equals {
                exported_name,
                exported_name_span,
                ..
            } => {
                check_export_assignment_expression(
                    surge_ts_syntax::ParsedExpression::Identifier {
                        name: exported_name,
                        span: exported_name_span,
                    },
                    ctx,
                );
            }
            ParsedExportDeclaration::EqualsExpression { expression, .. } => {
                check_export_assignment_expression(*expression, ctx);
            }
            ParsedExportDeclaration::NamespaceExport { .. } => {}
            ParsedExportDeclaration::Unsupported { span } => {
                let mut diagnostic =
                    Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        ParsedStatement::NamespaceDeclaration(namespace) => {
            check_namespace_body(&namespace, file_index, ctx);
        }
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, span);
        }
    }
}

/// tsc's `checkModuleDeclaration` checks the body as ordinary source
/// elements, in a scope of its own: the body's values (exported or not)
/// shadow the enclosing ones, and its bare type references resolve against the
/// namespace's qualified members.
///
/// The scope mirrors a module file's: hoisted function declarations are bound
/// up front, and the body's other values back later references through the
/// module value fallback, together with what other blocks of the same
/// namespace export.
fn check_namespace_body(
    namespace: &surge_ts_syntax::ParsedNamespaceDeclaration,
    file_index: usize,
    ctx: &mut CheckerContext,
) {
    let ambient_body;
    let namespace = if namespace.is_declare {
        ambient_body = ambient_namespace(namespace);
        &ambient_body
    } else {
        namespace
    };
    let prefix = match ctx.namespace_member_prefix_stack.last() {
        Some(outer) => format!("{outer}.{}", namespace.member_name()),
        None => namespace.name.clone(),
    };
    ctx.namespace_member_prefix_stack.push(prefix);

    let enclosing = std::sync::Arc::new(
        ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext),
    );
    let type_declarations = ctx.type_declarations.clone();
    let namespace_values = crate::modules::collect_exportable_value_symbols(
        &namespace.statements,
        &type_declarations,
        &enclosing,
        None,
        false,
        ctx,
    );
    let mut body_values = crate::symbols::SymbolTable::new();
    for (name, symbol) in namespace_values.iter_shared() {
        let inherited = enclosing
            .get_handle(name)
            .is_some_and(|outer| std::sync::Arc::ptr_eq(&outer, symbol));
        if !inherited {
            let _ = body_values.insert_shared(name.clone(), symbol.clone());
        }
    }
    let merged = (!namespace.name.contains('.'))
        .then(|| {
            enclosing.get_handle(&namespace.name).or_else(|| {
                ctx.module_value_fallback
                    .as_ref()
                    .and_then(|fallback| fallback.get_handle(&namespace.name))
            })
        })
        .flatten();
    if let Some(merged) = merged
        && let surge_ts_types::Type::Object(object) = &merged.ty
    {
        for (name, property) in object.properties.iter() {
            let own = body_values.get_own(name).map(|symbol| symbol.ty.clone());
            // Another block's export, or this block's nested namespace, which
            // the merged object carries with every block's members.
            if own.is_none() || matches!(own, Some(surge_ts_types::Type::Object(_))) {
                let _ = body_values.insert(
                    name.to_string(),
                    crate::symbols::SymbolInfo {
                        ty: property.ty.clone(),
                        kind: crate::symbols::SymbolKind::Var,
                        function_signature: None,
                    },
                );
            }
        }
    }
    bind_namespace_require_aliases(&namespace.statements, &mut body_values, ctx);
    let saved_fallback = ctx.module_value_fallback.take();
    let body_values = match saved_fallback.clone() {
        Some(outer) => body_values.with_parent_fallback(outer),
        None => body_values,
    };
    ctx.module_value_fallback = Some(std::sync::Arc::new(body_values));

    let mut symbols = crate::symbols::SymbolTable::declaration_scope(enclosing);
    let mut function_signatures = HashMap::new();
    collect_function_signatures_from_statements(
        &namespace.statements,
        file_index,
        &mut symbols,
        &mut function_signatures,
        ctx,
    );
    let saved_symbols = std::mem::take(&mut ctx.symbols);
    ctx.set_symbols(symbols);
    check_program_file_statements(&namespace.statements, file_index, &function_signatures, ctx);
    ctx.set_symbols(saved_symbols);
    ctx.module_value_fallback = saved_fallback;
    ctx.namespace_member_prefix_stack.pop();
}

/// tsc never collects a namespace's `import x = require("m")` (it is TS1147),
/// so resolving the alias finds only an ambient `declare module "m"`
/// (`tryFindAmbientModule`) and otherwise reports the module unresolved at the
/// specifier — once something names the alias, which is when tsc resolves it.
/// The alias is declared either way, error-typed when unresolved.
fn bind_namespace_require_aliases(
    statements: &[ParsedStatement],
    body_values: &mut crate::symbols::SymbolTable,
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        let import = match statement {
            ParsedStatement::ImportDeclaration(import) => import,
            ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
                ParsedExportDeclaration::Statement { declaration, .. } => match declaration.as_ref() {
                    ParsedStatement::ImportDeclaration(import) => import,
                    _ => continue,
                },
                _ => continue,
            },
            _ => continue,
        };
        let surge_ts_syntax::ParsedImportKind::Equals { local_name, .. } = &import.kind else {
            continue;
        };
        if body_values.get_own(local_name).is_some() {
            continue;
        }
        let symbol = crate::modules::ambient_module_export_table(ctx, &import.module_specifier).map(
            |table| match table.export_assignment_symbol.clone() {
                Some(symbol) => (*symbol).clone(),
                None => crate::symbols::SymbolInfo {
                    ty: crate::modules::namespace_export_object_type(table),
                    kind: crate::symbols::SymbolKind::Var,
                    function_signature: None,
                },
            },
        );
        match symbol {
            Some(symbol) => {
                let _ = body_values.insert(local_name.clone(), symbol);
            }
            None => {
                if ctx
                    .namespace_require_reads
                    .as_ref()
                    .is_some_and(|reads| reads.contains(local_name.as_str()))
                {
                    crate::modules::emit_unresolvable_module_reference(
                        ctx,
                        &import.module_specifier,
                        import.module_specifier_span.or(import.span),
                    );
                }
                crate::modules::insert_error_typed_value_import(local_name, body_values);
            }
        }
    }
}

/// The file's referenced names, when some namespace in it holds an
/// `import x = require()` (see [`bind_namespace_require_aliases`]).
pub(crate) fn namespace_require_reads(
    statements: &[ParsedStatement],
    module_reads: &[String],
) -> Option<std::sync::Arc<surge_ts_types::fx::FxHashSet<String>>> {
    fn has_namespace_require(statements: &[ParsedStatement], in_namespace: bool) -> bool {
        statements.iter().any(|statement| {
            let statement = match statement {
                ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
                    ParsedExportDeclaration::Statement { declaration, .. } => declaration.as_ref(),
                    _ => return false,
                },
                other => other,
            };
            match statement {
                ParsedStatement::NamespaceDeclaration(namespace) => {
                    has_namespace_require(&namespace.statements, true)
                }
                ParsedStatement::ImportDeclaration(import) => {
                    in_namespace && matches!(import.kind, surge_ts_syntax::ParsedImportKind::Equals { .. })
                }
                _ => false,
            }
        })
    }
    has_namespace_require(statements, false)
        .then(|| std::sync::Arc::new(module_reads.iter().cloned().collect()))
}

/// A `declare namespace` makes every declaration in it ambient, nested
/// namespaces and classes included (a `declare class` has no initializers to
/// check).
fn ambient_namespace(
    namespace: &surge_ts_syntax::ParsedNamespaceDeclaration,
) -> surge_ts_syntax::ParsedNamespaceDeclaration {
    fn ambient_statement(statement: &mut ParsedStatement) {
        match statement {
            ParsedStatement::ClassDeclaration(class) => class.is_declare = true,
            ParsedStatement::NamespaceDeclaration(inner) => {
                inner.is_declare = true;
                inner.statements.iter_mut().for_each(ambient_statement);
            }
            ParsedStatement::ExportDeclaration(export) => {
                if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_mut() {
                    ambient_statement(declaration);
                }
            }
            _ => {}
        }
    }
    let mut ambient = namespace.clone();
    ambient.statements.iter_mut().for_each(ambient_statement);
    ambient
}

/// The target of `export =` / `export default <expression>`, which tsc checks
/// as an ordinary expression except that its root name may name a type or a
/// namespace.
fn check_export_assignment_expression(
    expression: surge_ts_syntax::ParsedExpression,
    ctx: &mut CheckerContext,
) {
    if crate::checks::expr::export_assignment_target_is_exempt(&expression, ctx) {
        return;
    }
    expr::check_expression_statement(expression, ctx);
}

pub(crate) fn emit_unsupported_declaration_diagnostics(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        emit_unsupported_declaration_diagnostic_from_statement(statement, ctx);
    }
}

pub(crate) fn emit_unsupported_declaration_diagnostic_from_statement(
    statement: &ParsedStatement,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, *span);
        }
        ParsedStatement::ImportDeclaration(import)
            if matches!(
                import.kind,
                ParsedImportKind::Unsupported | ParsedImportKind::TypeOnlyDefault { .. }
            ) =>
        {
            emit_unsupported_declaration_diagnostic(
                ctx,
                import.span.or(import.module_specifier_span),
            );
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Unsupported { span }
            | ParsedExportDeclaration::EqualsExpression { span, .. } => {
                emit_unsupported_declaration_diagnostic(ctx, *span);
            }
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                span: declaration_span,
            } => {
                emit_unsupported_declaration_diagnostic(ctx, (*span).or(*declaration_span));
            }
            _ => {}
        },
        ParsedStatement::DeclareModuleDeclaration(module) => {
            emit_unsupported_declaration_diagnostics(&module.statements, ctx);
        }
        _ => {}
    }
}

pub(crate) fn emit_unsupported_declaration_diagnostic(
    ctx: &mut CheckerContext,
    span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::surge_unsupported_declaration(ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn check_program_function_declaration(
    function: ParsedFunctionDeclaration,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    let declaration_location = FunctionDeclarationLocation {
        file_index,
        statement_index,
    };

    let saved_symbols = std::mem::take(&mut ctx.symbols);
    let body_root_symbols =
        saved_symbols.clone_with_reason(surge_ts_types::TypeCopyReason::FunctionBodySetup);
    ctx.symbols = body_root_symbols;
    let Some(function_type) = function_signatures.get(&declaration_location) else {
        check_function::check_function_declaration(function, ctx);
        ctx.symbols = saved_symbols;
        return;
    };

    let type_parameters = function.type_parameters.clone();
    check_function::check_type_parameter_declarations(&type_parameters, ctx);
    check_function::check_function_declaration_body(function, function_type, &type_parameters, ctx);
    ctx.symbols = saved_symbols;
}

pub(crate) fn count_local_type_declarations_in_statements(statements: &[ParsedStatement]) -> usize {
    statements
        .iter()
        .map(count_local_type_declarations_in_statement)
        .sum()
}

pub(crate) fn count_local_type_declarations_in_statement(statement: &ParsedStatement) -> usize {
    match statement {
        ParsedStatement::TypeAliasDeclaration(_) => 1,
        ParsedStatement::InterfaceDeclaration(_) => 1,
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                count_local_type_declarations_in_statement(declaration.as_ref())
            }
            _ => 0,
        },
        _ => 0,
    }
}
