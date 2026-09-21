//! Per-statement checkers for function bodies (declarations, control flow,
//! assignments, returns) dispatched from [`super::check_function_body_statement`].

use super::*;

use surge_ts_syntax::{
    ParsedExpression, ParsedFunctionBodyStatement, ParsedVariableDeclaration, ParsedVariableKind,
};
use surge_ts_types::{Type, is_assignable_to};

use crate::checks::expr::evaluate_expression;
use crate::checks::var::{VariableCheckOptions, check_variable_declaration_against_symbols};
use crate::context::CheckerContext;
use crate::flow::{
    AssignmentState, FlowCheck, FunctionFlowState, apply_variable_declaration_state,
    check_expression_flow,
};
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

mod alias_conditions;
mod assignments;
mod branch_assignments;
mod control_flow;
mod returns;

pub(crate) use alias_conditions::*;
pub(crate) use assignments::*;
use branch_assignments::*;
pub(crate) use branch_assignments::branch_assigned_names;
pub(crate) use control_flow::*;
pub(crate) use returns::*;

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
        if matches!(variable_kind, ParsedVariableKind::Const) && is_condition_shaped(initializer) {
            let condition = std::sync::Arc::new(initializer.clone());
            flow_state.record_alias_guard_condition(local_name.clone(), condition.clone());
            scopes.record_alias_condition(local_name.as_str(), Some(condition));
        } else {
            scopes.record_alias_condition(local_name.as_str(), None);
        }
        // A `const` bound to a property reference is a discriminant alias:
        // `const { direction } = opts` lowers to `direction = opts.direction`,
        // and testing `direction` narrows `opts`.
        if matches!(variable_kind, ParsedVariableKind::Const) && is_property_reference(initializer)
        {
            flow_state.record_discriminant_alias(
                local_name.clone(),
                std::sync::Arc::new(initializer.clone()),
            );
        }
        // `const [error, value] = tuple` lowers to one `tuple[N]` binding per
        // element; remembering which element each one is makes the group
        // dependent when `tuple` is a union of tuples.
        let destructured = matches!(variable_kind, ParsedVariableKind::Const)
            .then(|| {
                tuple_destructure_binding(
                    initializer,
                    variable.array_pattern_span,
                    &visible_symbols(scopes),
                    ctx,
                )
            })
            .flatten();
        scopes.record_tuple_destructure(local_name.as_str(), destructured);
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

    let is_annotated = variable.declared_type.is_some();
    let probe_initializer = (literal_initializer_type.is_none()
        && variable.declared_type.is_some()
        && !initializer_flow_blocked)
        .then(|| variable.initializer.clone())
        .flatten();

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
        let narrowed = literal_initializer_type
            .filter(|initialized| {
                matches!(symbol.ty, Type::Union(_)) && is_assignable_to(initialized, &symbol.ty)
            })
            .or_else(|| {
                let Type::Union(union) = &symbol.ty else {
                    return None;
                };
                let assigned =
                    assigned_initializer_type(probe_initializer.as_ref()?, visible_symbols, ctx)?;
                let kept: Vec<Type> = union
                    .types()
                    .iter()
                    .filter(|member| is_assignable_to(&assigned, member))
                    .cloned()
                    .collect();
                (!kept.is_empty() && kept.len() < union.types().len())
                    .then(|| surge_ts_types::union_type(kept))
            });
        match narrowed {
            Some(initialized) => {
                let declared = symbol.ty.clone();
                scopes.insert_current_handle(local_name.as_str(), symbol);
                scopes.insert_current_declared(
                    local_name.as_str(),
                    SymbolInfo {
                        ty: initialized,
                        kind: symbol_kind_for_variable(variable_kind),
                        function_signature: None,
                    },
                    declared,
                );
            }
            // An annotation is the binding's declared type for good: recording
            // it lets a later write be checked against it and narrowed from
            // it, rather than rewriting the binding to whatever was assigned.
            None if is_annotated && !symbol.ty.is_unknown() => {
                let declared = symbol.ty.clone();
                let info = SymbolInfo {
                    ty: declared.clone(),
                    kind: symbol_kind_for_variable(variable_kind),
                    function_signature: symbol.function_signature.clone(),
                };
                scopes.insert_current_handle(local_name.as_str(), symbol);
                scopes.insert_current_declared(local_name.as_str(), info, declared);
            }
            None => {
                scopes.insert_current_handle(local_name.as_str(), symbol);
            }
        }
    }
}

/// tsc narrows an annotated union declaration by whatever it is initialized
/// with, not only by a literal: `let client: PC | undefined = persisted` starts
/// out as `PC`. Only a cleanly inferred initializer proves anything — a written
/// `unknown` is not a degradation, so a type that carries one (`{ [k: string]:
/// unknown }`) still narrows. The initializer was already checked against the
/// annotation; this re-evaluation runs without that contextual type, so
/// whatever it reports (an implicit-any method parameter, say) is a probe
/// artifact and is discarded.
fn assigned_initializer_type(
    initializer: &ParsedExpression,
    visible_symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let checkpoint = ctx.diagnostics().len();
    let inferred = crate::infer::infer_expression(initializer, visible_symbols, ctx);
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    // Shallow on purpose: a nominal reference to an imported interface is a
    // clean answer even when a member deep inside it resolved to the sentinel,
    // and the deep walk refused every `let client: PersistedClient | undefined
    // = persistedClient` in the persister packages, leaving the binding at its
    // annotation.
    match inferred {
        InferredExpression::Known(ty)
            if !ty.is_unknown()
                && !matches!(ty, Type::Any)
                && !initializer_type_is_degraded(&ty) =>
        {
            Some(ty)
        }
        _ => None,
    }
}

fn initializer_type_is_degraded(ty: &Type) -> bool {
    match ty {
        Type::Unknown | Type::TypeParameter(_) => true,
        Type::Union(union) => union.types().iter().any(initializer_type_is_degraded),
        _ => false,
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
        ParsedExpression::NumberLiteral(value) => {
            Some(Type::NumberLiteral(surge_ts_types::NumberLiteralType {
                value: value.clone(),
            }))
        }
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
    // A bare block always runs, so what it assigns to an outer binding holds
    // after it; a binding the block declares itself is not the outer one.
    let mut assigned = Vec::new();
    branch_assigned_names(&block_body, &mut assigned);
    assigned.retain(|name| {
        !block_body.iter().any(|statement| {
            matches!(
                statement,
                ParsedFunctionBodyStatement::VariableDeclaration(variable) if variable.name == *name
            )
        })
    });
    scopes.push_child();
    check_function_body(block_body, return_type, scopes, flow_state, ctx);
    let assigned_types = branch_assignment_types(&assigned, scopes);
    scopes.pop_child();
    adopt_branch_assignments(&assigned_types, scopes);
}

pub(crate) fn check_function_expression_statement(
    expression: ParsedExpression,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
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

    // `assertIsObject(obj);` narrows from here to the end of the block, not
    // inside a branch, so the narrowing is applied at the statement.
    crate::checks::function::narrow_assertion_call_in_scope(&expression, scopes, ctx);
}
