//! Per-statement program checking and unsupported-declaration diagnostics.

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
    for (statement_index, statement) in statements.iter().cloned().enumerate() {
        let statement = expand_module_if_alias(statement, &statements[..statement_index]);
        check_program_statement(
            statement,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        );
    }
}

/// `const ok = typeof v === "string"; if (ok) …` narrows by the condition the
/// alias was written as, as a function body does (tsc's aliased-condition
/// narrowing). Module scope keeps no flow state to record the alias in, so the
/// `const` is looked up among the statements before the `if`; its condition is
/// only ever used for narrowing, since a module `if` discards the condition's
/// diagnostics.
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
    // The condition is evaluated for its narrowing only: a condition that reads
    // a value surge models more loosely than tsc (`args.verbose` off a
    // parsed-options object that degraded to an index signature) would report
    // where tsc does not. Its diagnostics are discarded like a probe's.
    let checkpoint = ctx.diagnostics().len();
    let _ = crate::checks::expr::evaluate_expression(
        &if_statement.condition,
        if_statement.condition_span,
        &symbols,
        ctx,
    );
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
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
    let flow_facts = crate::flow::collect_function_flow_facts(&statements);
    let mut flow_state = crate::flow::FunctionFlowState::new(
        flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
    );
    crate::checks::function::check_function_body(statements, None, &mut scopes, &mut flow_state, ctx);
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

pub(crate) fn check_program_statement(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            var::check_variable_declaration(*variable, ctx);
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
            crate::checks::function::check_member_assignment(*assignment, &mut scopes, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            with_declared_mutable_module_bindings(ctx, |ctx| {
                check_program_function_declaration(
                    *function,
                    file_index,
                    statement_index,
                    function_signatures,
                    ctx,
                );
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
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ClassDeclaration(class) => {
            super::check_class_declaration(&class, ctx);
        }
        ParsedStatement::ImportDeclaration(_) => {}
        ParsedStatement::ExportDeclaration(export) => match *export {
            ParsedExportDeclaration::Statement { declaration, .. } => check_program_statement(
                *declaration,
                file_index,
                statement_index,
                function_signatures,
                ctx,
            ),
            ParsedExportDeclaration::Named { .. } => {}
            ParsedExportDeclaration::Namespace { .. } => {}
            ParsedExportDeclaration::Default { declaration, .. } => match declaration {
                ParsedDefaultExportDeclaration::Function(function) => {
                    check_program_function_declaration(
                        function,
                        file_index,
                        statement_index,
                        function_signatures,
                        ctx,
                    );
                }
                ParsedDefaultExportDeclaration::Expression(expression) => {
                    expr::check_expression_statement(expression, ctx);
                }
                ParsedDefaultExportDeclaration::Class(class) => {
                    super::check_class_declaration(&class, ctx);
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
            ParsedExportDeclaration::Equals { .. } => {}
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
        // Namespace members are bound during type-declaration collection; the
        // namespace itself produces no value-level checks here.
        ParsedStatement::NamespaceDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, span);
        }
    }
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
            ParsedExportDeclaration::Unsupported { span } => {
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
