
use surge_ts_syntax::{
    ParsedExpression,
    ParsedFunctionBodyStatement,
};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason};

use crate::context::CheckerContext;
use crate::symbols::{ScopeStack, SymbolInfo};
use super::super::narrow_discriminant_in_scope;

/// The bindings a branch body assigns at its own statement level. Deeper
/// assignments are discarded with their own inner frame before the branch ends,
/// so they cannot reach the join.
/// The identifier a reference path starts from: `a` for `a.b.c` and `a[0].b`.
/// `None` for a path rooted in anything that is not a plain binding.
fn reference_root_name(expression: &ParsedExpression) -> Option<&str> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name),
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. }
        | ParsedExpression::ElementAccess { object, .. } => reference_root_name(object),
        ParsedExpression::IndexAccess { object_name, .. } => Some(object_name),
        _ => None,
    }
}

pub(crate) fn branch_assigned_names(body: &[ParsedFunctionBodyStatement], names: &mut Vec<String>) {
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::Assignment(assignment) => {
                if !names.iter().any(|name| *name == assignment.target_name) {
                    names.push(assignment.target_name.clone());
                }
            }
            // `o.p = v` narrows by rewriting the *base* symbol's type
            // (`narrow_reference_in_scope`), so the name to carry across the
            // branch join is the root of the reference path, not the member.
            ParsedFunctionBodyStatement::MemberAssignment(assignment) => {
                if let Some(base) = reference_root_name(&assignment.target)
                    && !names.iter().any(|name| name == base)
                {
                    names.push(base.to_string());
                }
            }
            ParsedFunctionBodyStatement::Block(block) => branch_assigned_names(block, names),
            _ => {}
        }
    }
}

/// Every plain binding a loop body assigns, at any depth. Unlike a branch, a
/// loop's nested assignments reach its head through the back edge.
fn loop_assigned_names(body: &[ParsedFunctionBodyStatement], names: &mut Vec<String>) {
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::Assignment(assignment) => {
                if !names.iter().any(|name| *name == assignment.target_name) {
                    names.push(assignment.target_name.clone());
                }
            }
            ParsedFunctionBodyStatement::Block(block) => loop_assigned_names(block, names),
            ParsedFunctionBodyStatement::If(if_statement) => {
                loop_assigned_names(&if_statement.then_body, names);
                loop_assigned_names(&if_statement.else_body, names);
            }
            ParsedFunctionBodyStatement::While(while_statement) => {
                loop_assigned_names(&while_statement.body, names);
            }
            ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
                loop_assigned_names(&for_of_statement.body, names);
            }
            ParsedFunctionBodyStatement::Switch(switch_statement) => {
                for case in &switch_statement.cases {
                    loop_assigned_names(&case.consequent, names);
                }
            }
            ParsedFunctionBodyStatement::Try(try_statement) => {
                loop_assigned_names(&try_statement.block, names);
                if let Some(handler) = &try_statement.handler {
                    loop_assigned_names(&handler.body, names);
                }
                loop_assigned_names(&try_statement.finalizer, names);
            }
            _ => {}
        }
    }
}

/// tsc types a binding at a loop head as the union of the entry edge and every
/// back edge. surge checks a loop body once, so a binding the body reassigns
/// starts the body at its declared type rather than at whatever it was narrowed
/// to on entry — otherwise `let min: number | null = null` read as `null`
/// throughout a loop that assigns it.
pub(super) fn widen_loop_assigned_bindings(
    body: &[ParsedFunctionBodyStatement],
    scopes: &mut ScopeStack,
) {
    let mut names = Vec::new();
    loop_assigned_names(body, &mut names);
    for name in names {
        let Some(symbol) = scopes.resolve(&name) else {
            continue;
        };
        let Some(declared) = scopes.visible_symbols().declared_type(&name).cloned() else {
            continue;
        };
        if declared == symbol.ty || declared.is_unknown() {
            continue;
        }
        let widened = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
            union_type(vec![symbol.ty.clone(), declared])
        });
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let _ = scopes.update_visible(
            &name,
            SymbolInfo {
                ty: widened,
                kind,
                function_signature,
            },
        );
    }
}

/// Joins a then-branch's end types with the fall-through (condition-false) types
/// for the bindings it assigned. tsc types the code after `if (!x) { x = … }`
/// from both incoming edges; surge's branch scope discards the assignment
/// narrowing on `pop_child`, which otherwise leaves the declared union in place
/// for every later use. The join only ever removes union members both edges
/// rule out — a widening result is a modelling artifact and is dropped.
pub(super) fn join_branch_assignments(
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
pub(super) fn join_branch_pair(
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

/// Adopts a branch's end types outright, for a join the branch is the *only*
/// incoming edge of. A `try` whose `catch` returns or throws is that shape:
/// reaching the statement's end means the block ran to completion, so the
/// assignments it made hold from there on. Without this the narrowing dies with
/// the branch frame and a `let x: T | undefined` assigned inside the `try` reads
/// as possibly-undefined for the rest of the function.
pub(super) fn adopt_branch_assignments(branch_types: &[(String, Type)], scopes: &mut ScopeStack) {
    for (name, branch_ty) in branch_types {
        let Some(symbol) = scopes.resolve(name) else {
            continue;
        };
        if *branch_ty == symbol.ty {
            continue;
        }
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let bound = scopes
            .visible_symbols()
            .declared_type(name)
            .cloned()
            .unwrap_or_else(|| symbol.ty.clone());
        if !is_assignable_to(branch_ty, &bound) {
            continue;
        }
        let _ = scopes.update_visible(
            name,
            SymbolInfo {
                ty: branch_ty.clone(),
                kind,
                function_signature,
            },
        );
    }
}

/// Snapshots the current type of each assigned binding at a branch's end, before
/// its scope frame pops.
pub(super) fn branch_assignment_types(names: &[String], scopes: &ScopeStack) -> Vec<(String, Type)> {
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
