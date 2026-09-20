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
pub(crate) mod evolving_arrays;
mod returns;

pub(crate) use alias_conditions::*;
pub(crate) use assignments::*;
pub(crate) use evolving_arrays::{apply_array_mutations, collect_array_mutations};
use evolving_arrays::{prime_loop_mutations, release_loop_mutations};
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
    // An annotation naming an in-scope type parameter resolves to the body's
    // placeholder, which reads as degraded; definite assignment needs to know
    // it is `T` (see `type_assumed_initialized`).
    let annotated_type_parameter = match &variable.declared_type {
        Some(surge_ts_syntax::ParsedType::Named(named))
            if named.type_arguments.is_empty() && ctx.type_parameter_in_scope(&named.name) =>
        {
            Some(Type::TypeParameter(surge_ts_types::TypeParameterType {
                name: named.name.as_str().into(),
            }))
        }
        _ => None,
    };
    let has_initializer = variable.initializer.is_some();
    let contextual_annotation = variable.declared_type.is_some() && annotated_type_parameter.is_none();
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
        if matches!(variable_kind, ParsedVariableKind::Const)
            && let Some((source, index)) = tuple_destructure_source(initializer)
        {
            flow_state.record_tuple_destructure_binding(local_name.clone(), source, index);
        }
    }

    check_local_duplicate_declaration(&variable, scopes, ctx);
    let auto_declaration = evolving_arrays::auto_declaration(&variable, ctx);
    let auto_variable = auto_declaration.map(|_| ParsedVariableDeclaration {
        declared_type: None,
        initializer: variable
            .initializer
            .as_ref()
            .filter(|initializer| {
                matches!(
                    initializer,
                    ParsedExpression::UndefinedLiteral | ParsedExpression::NullLiteral
                )
            })
            .cloned()
            .or_else(|| variable.initializer.as_ref().map(|_| ParsedExpression::Unknown)),
        ..variable.clone()
    });

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

        // A written annotation other than a bare type parameter is a
        // non-generic contextual type for the initializer.
        let blocked = if contextual_annotation {
            crate::flow::check_substituting_read_flow(
                initializer,
                variable.initializer_span,
                flow_state,
                statement_index,
                ctx,
            )
        } else {
            check_expression_flow(
                initializer,
                variable.initializer_span,
                flow_state,
                statement_index,
                ctx,
            )
        }
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

    let has_written_annotation = variable.declared_type.is_some();
    let literal_initializer_type = matches!(
        variable_kind,
        ParsedVariableKind::Let | ParsedVariableKind::Var | ParsedVariableKind::Const
    )
    .then(|| literal_initializer_type(variable.initializer.as_ref()))
    .flatten();

    let visible_symbols = visible_symbols(scopes);

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
            Some(annotated_type_parameter.as_ref().unwrap_or(&symbol.ty)),
            flow_state,
            ctx,
        );
        if let Some(Type::TypeParameter(parameter)) = &annotated_type_parameter
            && crate::flow::constraint_substitutes(&parameter.name, ctx)
        {
            flow_state.constraint_exempt.insert(local_name.as_str().into());
        }
        // A declared union narrows to what the initializer can inhabit, exactly
        // as a later assignment does: `let style: Style = "simple"` is
        // `"simple"` until reassigned, and `let v: string | number = "a"` is
        // `string`. Recorded as a *narrowing* so a later
        // assignment still checks against the declaration. Only a literal
        // initializer participates — its type is known without re-evaluating
        // (and re-reporting) the expression.
        let narrowed = literal_initializer_type
            .filter(|initialized| {
                matches!(symbol.ty, Type::Union(_)) && is_assignable_to(initialized, &symbol.ty)
            })
            .map(|initialized| assignments::assignment_reduced_type(Some(&symbol.ty), initialized))
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
                // An un-annotated `let`/`var` is declared at its initializer's
                // *widened* type (`getWidenedTypeForVariableLikeDeclaration`),
                // and a later assignment narrows within that — `let status =
                // state.status` stays the union a written `"success"` selects
                // from instead of widening to `string`.
                // Only a *union* declaration is recorded: that is the shape a
                // later assignment narrows within (`assignment_reduced_type`).
                // An `undefined`/auto initializer must stay unbound — it is
                // still evolving, and binding it would reject the assignments
                // that give it its type.
                let declared = (!has_written_annotation
                    && matches!(variable_kind, ParsedVariableKind::Let | ParsedVariableKind::Var)
                    && matches!(symbol.ty, Type::Union(_)))
                .then(|| symbol.ty.clone());
                match declared {
                    Some(declared) => {
                        scopes.insert_current_narrowed(
                            local_name.as_str(),
                            SymbolInfo {
                                ty: declared.clone(),
                                kind: symbol_kind_for_variable(variable_kind),
                                function_signature: symbol.function_signature.clone(),
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
        match &auto_variable {
            Some(declaration) => evolving_arrays::declare_auto_binding(
                declaration,
                auto_declaration,
                scopes,
                ctx,
            ),
            None => scopes.declare_auto_array(local_name.as_str(), None),
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
    scopes.push_child();
    check_function_body(block_body, return_type, scopes, flow_state, ctx);
    scopes.pop_child();
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
