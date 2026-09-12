
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
pub(super) fn branch_assigned_names(body: &[ParsedFunctionBodyStatement], names: &mut Vec<String>) {
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::Assignment(assignment) => {
                if !names.iter().any(|name| *name == assignment.target_name) {
                    names.push(assignment.target_name.clone());
                }
            }
            ParsedFunctionBodyStatement::Block(block) => branch_assigned_names(block, names),
            _ => {}
        }
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
