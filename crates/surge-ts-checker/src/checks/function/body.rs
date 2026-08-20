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
    // Peel lazy named references so the exemption sees what the annotation
    // resolves to: `Promise<T>` is modeled as its awaited `T` (implicit await),
    // so an async `(): Promise<void>` body with no `return` is exempt exactly
    // like `(): void`, and a named alias of `void`/`undefined`/`any` is exempt
    // like the keyword itself (tsc treats aliases as transparent here).
    let peeled;
    let return_type = if matches!(return_type, Type::Reference(_)) {
        peeled = return_type.peeled();
        &peeled
    } else {
        return_type
    };
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

    // Body-local `type`/`interface`/`class` declarations bind a block-scoped
    // type: install them as the innermost type-declaration layer for the whole
    // body (they are visible before their statement position, like the function
    // hoist below).
    let saved_type_declaration_scope = install_body_local_type_declarations(&body, scopes, ctx);

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
                    function_signature: Some(
                        crate::checks::function::signature::function_signature_info(
                            &function.type_parameters,
                            &function.parameters,
                            function.return_type.as_ref(),
                            &ctx.file_name,
                        ),
                    ),
                }),
            );
        }
    }

    // A body-local type declaration's own body may name a body-local *value*
    // (`type Schema = Infer<typeof schema>`), and the `typeof` arm resolves
    // names through `ctx.symbols` — a file-level table that never holds function
    // locals. Republish the body's visible value scope there for the duration of
    // the statement loop so the declaration resolves wherever it is forced from.
    // Restricted to bodies that actually declare a local type: the republished
    // table is shared with the scope stack, so every subsequent local binding
    // copies the (small) locals map.
    let saved_symbols = saved_type_declaration_scope
        .is_some()
        .then(|| std::mem::take(&mut ctx.symbols));

    for (statement_index, statement) in body.into_iter().enumerate() {
        if saved_symbols.is_some() {
            ctx.symbols = scopes.visible_symbols().clone();
        }
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

    if let Some(saved) = saved_symbols {
        ctx.symbols = saved;
    }

    if let Some(saved) = saved_type_declaration_scope {
        ctx.type_declaration_scope = saved;
    }
}

/// Binds the body's own `type`/`interface`/`class` declarations as an inner
/// type-declaration layer (and, for classes, a local value symbol), returning
/// the scope to restore once the body has been checked.
///
/// Returns `None` — installing nothing — for the overwhelmingly common body
/// that declares no local types.
fn install_body_local_type_declarations(
    body: &[ParsedFunctionBodyStatement],
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) -> Option<Option<std::sync::Arc<crate::symbols::TypeDeclarationScope>>> {
    if !body.iter().any(|statement| {
        matches!(
            statement,
            ParsedFunctionBodyStatement::TypeAlias(_)
                | ParsedFunctionBodyStatement::Interface(_)
                | ParsedFunctionBodyStatement::Class(_)
        )
    }) {
        return None;
    }

    let declarations = collect_body_local_type_declarations(body, ctx);

    let outer_layers: Vec<std::sync::Arc<crate::symbols::TypeDeclarationTable>> = ctx
        .type_declaration_scope
        .as_ref()
        .map(|scope| scope.layers().to_vec())
        .unwrap_or_default();

    // Each declaration's own body resolves against the declarations that
    // precede it plus a placeholder layer, never against a scope that transitively
    // contains the declaration itself: a scope that reached back into the layer
    // holding it would close an `Arc` cycle and leak the layer for the run.
    // Forward/self references and the enclosing function's type parameters land
    // on the placeholder layer instead, degrading to `unknown` rather than
    // reporting an unresolved name.
    let placeholder_layer = std::sync::Arc::new(body_local_placeholder_table(&declarations, ctx));
    let mut prefix = crate::symbols::TypeDeclarationTable::new();
    let mut body_layer = crate::symbols::TypeDeclarationTable::new();
    for (name, declaration) in declarations {
        let mut layers = Vec::with_capacity(outer_layers.len() + 2);
        layers.push(std::sync::Arc::new(prefix.clone()));
        layers.push(placeholder_layer.clone());
        layers.extend(outer_layers.iter().cloned());
        let scope = std::sync::Arc::new(crate::symbols::TypeDeclarationScope::new(layers));

        let declaration = with_resolution_scope(declaration, scope);
        let _ = prefix.insert(name.as_str(), declaration.clone());
        let _ = body_layer.insert(name.as_str(), declaration);
    }

    let mut layers = Vec::with_capacity(outer_layers.len() + 1);
    layers.push(std::sync::Arc::new(body_layer));
    layers.extend(outer_layers);

    let saved = ctx.type_declaration_scope.take();
    ctx.type_declaration_scope = Some(std::sync::Arc::new(
        crate::symbols::TypeDeclarationScope::new(layers),
    ));

    // The real static type is built at the class's own statement position, once
    // the values its heritage clause may name (`class D extends Parent {}` over a
    // preceding `const Parent = …`) are bound. Reserve the name here so a read
    // from an earlier closure degrades to `any` instead of reporting TS2304.
    for statement in body {
        if let ParsedFunctionBodyStatement::Class(class) = statement {
            scopes.insert_current_handle(
                class.name.as_str(),
                std::sync::Arc::new(SymbolInfo {
                    ty: Type::Any,
                    kind: crate::symbols::SymbolKind::Const,
                    function_signature: None,
                }),
            );
        }
    }

    Some(saved)
}

/// The body-local declarations of `body`, paired with the name each binds.
///
/// A declaration is registered under a synthetic, declaration-site-unique
/// internal name (`T@<offset>`) with the source name kept as its display name:
/// the program-wide resolution caches are keyed on `(file, declaration name)`,
/// and two sibling function bodies in one file may legitimately declare the same
/// local type with different bodies (zod's test files do exactly that).
fn collect_body_local_type_declarations(
    body: &[ParsedFunctionBodyStatement],
    ctx: &mut CheckerContext,
) -> Vec<(String, crate::symbols::TypeDeclarationInfo)> {
    use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

    let file_name = ctx.file_name_arc();
    let mut declarations = Vec::new();

    for statement in body {
        let declaration = match statement {
            ParsedFunctionBodyStatement::TypeAlias(alias) => {
                let info = TypeAliasInfo::new(
                    body_local_declaration_name(&alias.name, alias.name_span),
                    file_name.clone(),
                    alias.name_span,
                    alias.type_parameters.clone(),
                    alias.ty.clone(),
                    None,
                );
                (alias.name.clone(), TypeDeclarationInfo::Alias(info))
            }
            ParsedFunctionBodyStatement::Interface(interface) => {
                let info = InterfaceInfo::new(
                    body_local_declaration_name(&interface.name, interface.name_span),
                    file_name.clone(),
                    interface.name_span,
                    interface.type_parameters.clone(),
                    interface.extends.clone(),
                    interface.members.clone(),
                    interface.string_index_type.clone(),
                    interface.call_signature.clone(),
                    interface.construct_signatures.clone(),
                    None,
                );
                (interface.name.clone(), TypeDeclarationInfo::Interface(info))
            }
            ParsedFunctionBodyStatement::Class(class) => {
                let mut info =
                    crate::program::class_instance_interface_info(class, file_name.clone());
                info.name = body_local_declaration_name(&class.name, class.name_span).into();
                (class.name.clone(), TypeDeclarationInfo::Interface(info))
            }
            _ => continue,
        };

        let (name, mut info) = declaration;
        set_declared_name(&mut info, &name);
        declarations.push((name, info));
    }

    declarations
}

/// The degradation layer a body-local declaration's own body resolves against:
/// the enclosing function's type parameters plus every body-local name,
/// each bound to the `unknown` sentinel. Without it a body-local alias over an
/// outer type parameter (`type TError = ClientError<TRouter>`) would merely move
/// the unresolved-name report from the use sites to the declaration.
fn body_local_placeholder_table(
    declarations: &[(String, crate::symbols::TypeDeclarationInfo)],
    ctx: &mut CheckerContext,
) -> crate::symbols::TypeDeclarationTable {
    use crate::symbols::{TypeAliasInfo, TypeDeclarationInfo};

    let file_name = ctx.file_name_arc();
    let anchor = declarations
        .iter()
        .find_map(|(_, declaration)| body_local_declaration_anchor(declaration));
    let mut table = crate::symbols::TypeDeclarationTable::new();

    let placeholder = |name: &str, table: &mut crate::symbols::TypeDeclarationTable| {
        let internal_name = match anchor {
            Some(anchor) => format!("{name}@{anchor}"),
            None => name.to_string(),
        };
        let mut info = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            internal_name,
            file_name.clone(),
            None,
            Vec::new(),
            surge_ts_syntax::ParsedType::Unknown,
            None,
        ));
        set_declared_name(&mut info, name);
        let _ = table.insert(name, info);
    };

    for (name, _) in declarations {
        placeholder(name, &mut table);
    }
    for scope in &ctx.type_parameter_scopes {
        for name in scope.keys() {
            placeholder(name, &mut table);
        }
    }

    table
}

fn body_local_declaration_anchor(
    declaration: &crate::symbols::TypeDeclarationInfo,
) -> Option<usize> {
    match declaration {
        crate::symbols::TypeDeclarationInfo::Alias(alias) => alias.name_span,
        crate::symbols::TypeDeclarationInfo::Interface(interface) => interface.name_span,
    }
    .map(|span| span.start)
}

fn body_local_declaration_name(name: &str, name_span: Option<surge_ts_syntax::TextSpan>) -> String {
    match name_span {
        Some(span) => format!("{name}@{}", span.start),
        None => name.to_string(),
    }
}

fn set_declared_name(declaration: &mut crate::symbols::TypeDeclarationInfo, name: &str) {
    match declaration {
        crate::symbols::TypeDeclarationInfo::Alias(alias) => {
            alias.declared_name = Some(std::sync::Arc::from(name));
        }
        crate::symbols::TypeDeclarationInfo::Interface(interface) => {
            interface.declared_name = Some(std::sync::Arc::from(name));
        }
    }
}

fn with_resolution_scope(
    declaration: crate::symbols::TypeDeclarationInfo,
    scope: std::sync::Arc<crate::symbols::TypeDeclarationScope>,
) -> crate::symbols::TypeDeclarationInfo {
    match declaration {
        crate::symbols::TypeDeclarationInfo::Alias(mut alias) => {
            alias.resolution_scope = Some(scope);
            crate::symbols::TypeDeclarationInfo::Alias(alias)
        }
        crate::symbols::TypeDeclarationInfo::Interface(mut interface) => {
            interface.resolution_scope = Some(scope);
            crate::symbols::TypeDeclarationInfo::Interface(interface)
        }
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
        // The type side is bound ahead of the statement loop by
        // `install_body_local_type_declarations`; a class's member bodies are
        // not separately checked, matching the nested-function treatment above.
        ParsedFunctionBodyStatement::TypeAlias(_) | ParsedFunctionBodyStatement::Interface(_) => {}
        ParsedFunctionBodyStatement::Class(class) => {
            let symbol = crate::program::build_class_value_symbol(&class, ctx);
            scopes.insert_current_handle(class.name.as_str(), std::sync::Arc::new(symbol));
        }
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
        ParsedFunctionBodyStatement::MemberAssignment(assignment) => {
            let start = Instant::now();
            check_member_assignment(*assignment, scopes, ctx);
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
