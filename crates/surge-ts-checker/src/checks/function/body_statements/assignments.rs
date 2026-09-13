
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedAssignment, ParsedExpression, ParsedMemberAssignment, ParsedThisPropertyAssignment,
    ParsedType,
};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason};

use crate::checks::assign::check_assignment_with_symbols;
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    FlowCheck, FunctionFlowState, check_assignment_target_flow, check_expression_flow, mark_assignment_state,
};
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};
use super::super::visible_symbols;

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

/// Checks an `o.p = v` assignment against the target property's declared type
/// and narrows the target for the code that follows. Nothing is reported when
/// either side carries the degradation sentinel, and the narrowing is
/// block-scoped exactly like the identifier-assignment one above.
/// The declared (un-narrowed) type of an `a.b.c` reference: the base symbol's
/// declared type walked through the written members, so a write to
/// `spec.imported.name` after `spec.imported.name === "x"` narrowed the read
/// is still checked against the property's declaration.
fn declared_reference_type(object: &ParsedExpression, symbols: &SymbolTable) -> Option<Type> {
    match object {
        ParsedExpression::Identifier { name, .. } => symbols.declared_type(name).cloned(),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => declared_reference_type(object, symbols)?.get_property_access_type(property_name),
        _ => None,
    }
}

/// The declaration behind a narrowed object (`Identifier` with `name` read as
/// `"x"` after `=== "x"`), re-resolved so a write sees the declared member.
/// Only a non-generic named declaration can be recovered from the display name.
fn declared_object_type_by_name(object_type: &Type, ctx: &mut CheckerContext) -> Option<Type> {
    let Type::Object(object) = object_type.peeled() else {
        return None;
    };
    let name = object.alias_name.as_deref()?;
    if name.contains(['<', ' ', '|', '&', '{']) {
        return None;
    }
    ctx.lookup_type_declaration(name)?;
    let declared = crate::infer::map_parsed_type(
        ParsedType::Named(std::sync::Arc::new(surge_ts_syntax::ParsedNamedType {
            name: name.to_string(),
            span: None,
            type_arguments: Vec::new(),
        })),
        ctx,
    );
    (!declared.is_unknown()).then_some(declared)
}

/// A read narrowed to a literal (`o.kind === "a"`) still writes against the
/// declared primitive; when the declaration itself is out of reach, the
/// literal is widened to its base so the write is not held to the narrowing.
fn widen_narrowed_literals(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Union(union) => {
            union_type(union.types().iter().map(widen_narrowed_literals).collect())
        }
        other => other.clone(),
    }
}

pub(crate) fn check_member_assignment(
    assignment: ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let ParsedExpression::PropertyAccess {
        object,
        object_span,
        property_name,
        property_span,
        ..
    } = &assignment.target
    else {
        return;
    };

    let visible_symbols = visible_symbols(scopes);

    // The receiver of the write is still read: `decl.init.callee = x` reports
    // a possibly-undefined `decl` and a missing `init` on it exactly as a read
    // would. The written member itself is only reported below on a union
    // receiver, so a value surge models incompletely
    // (`Component.getInitialProps = …`) is not a false TS2339 here.
    let object_type = match evaluate_expression(
        object,
        object_span.or(assignment.target_span),
        &visible_symbols,
        ctx,
    ) {
        InferredExpression::Known(ty) => ty,
        _ => return,
    };
    // A write through a possibly-undefined receiver (`decl.id = x` with
    // `decl` from an indexed read) is the same TS18048 a read reports.
    let object_type = crate::checks::expr::strip_reported_undefined_receiver(
        object,
        object_type,
        *object_span,
        assignment.target_span,
        &visible_symbols,
        ctx,
    );
    let _ = object_span;

    // A write checks against the property's *declared* type: after
    // `if (o.flag === undefined)` the read type is narrowed to `undefined`, but
    // `o.flag = true` is still an assignment to `boolean | undefined`.
    let declared_object_type = declared_reference_type(object, &visible_symbols)
        .or_else(|| declared_object_type_by_name(&object_type, ctx));
    let Some(target_type) = declared_object_type
        .as_ref()
        .and_then(|declared| declared.get_property_access_type(property_name))
        .or_else(|| {
            object_type
                .get_property_access_type(property_name)
                .map(|narrowed| widen_narrowed_literals(&narrowed))
        })
    else {
        // `decl.id = x` on `A | B` where `B` has no `id`: tsc reports the
        // member on the union. Only a union of fully modelled objects is
        // reported, so an incompletely modelled receiver stays silent.
        let receiver = declared_object_type.as_ref().unwrap_or(&object_type);
        if let Type::Union(union) = receiver.peeled()
            && union.types().iter().all(|member| {
                matches!(member.peeled(), Type::Object(_))
                    && !crate::checks::expr::carries_leaked_type_parameter(member, ctx)
            })
        {
            let diagnostic = crate::checks::expr::missing_property_diagnostic(
                property_name,
                receiver,
                &visible_symbols,
                ctx.file_name.clone(),
            );
            let span = property_span.or(assignment.target_span);
            ctx.push(match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            });
        }
        return;
    };

    let inferred_value = crate::checks::expected::evaluate_expression_with_expected_type(
        &assignment.value,
        assignment.value_span,
        Some(&target_type),
        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
        &visible_symbols,
        ctx,
    );

    let InferredExpression::Known(value_type) = inferred_value else {
        return;
    };

    if value_type.is_unknown() || target_type.is_unknown() {
        return;
    }

    if !is_assignable_to(&value_type, &target_type) {
        let diagnostic = Diagnostic::ts2322(
            &crate::checks::expr::source_display_name(&value_type, &target_type),
            &target_type.name(),
            ctx.file_name.clone(),
        );
        let diagnostic = match assignment.target_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return;
    }

    crate::checks::function::narrowing::narrow_assignment_target_in_scope(
        &assignment.target,
        &value_type,
        scopes,
    );
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

    // A write checks against the property's *declared* type, as
    // `check_member_assignment` does for `o.p = …`: inside
    // `if (this.value === "valid") this.value = "dirty"` the read type of
    // `this.value` is narrowed to `"valid"`, but the write still targets
    // `"aborted" | "dirty" | "valid"`.
    let Some(property_type) = visible_symbols
        .declared_type("this")
        .and_then(|declared| declared.get_property_access_type(&assignment.property_name))
        .or_else(|| {
            this_symbol
                .ty
                .get_property_access_type(&assignment.property_name)
        })
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

    if value_type.is_unknown() || property_type.is_unknown() {
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

    if value_ty.is_unknown() {
        return;
    }

    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };

    let mut narrowed_by_assignment = false;
    let updated_ty = if symbol.ty == Type::Undefined {
        union_type(vec![
            Type::Undefined,
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || value_ty.clone()),
        ])
    } else if symbol.ty == value_ty || is_assignable_to(&value_ty, &symbol.ty) {
        // Assigning to a union-declared variable narrows it to what was
        // assigned, as tsc does: the lazy-singleton idiom
        // (`let client: Redis | null = null; … client = new Redis(); return client;`)
        // otherwise keeps reading as the full union at every later use.
        if matches!(symbol.ty, Type::Union(_)) && !value_ty.is_unknown() {
            narrowed_by_assignment = true;
            value_ty
        } else {
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
        }
    } else if matches!(symbol.ty, Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_)) {
        union_type(vec![
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
            value_ty,
        ])
    } else if scopes
        .visible_symbols()
        .declared_type(target_name)
        .is_some_and(|declared| {
            matches!(declared, Type::Union(_)) && is_assignable_to(&value_ty, declared)
        })
    {
        // The binding is already narrowed (by its initializer, or by an earlier
        // assignment) to something this value does not inhabit. The assignment is
        // still legal against the *declaration*, and it re-narrows to the new
        // value — `let s: Wide = "a"; s = "c";` is `"c"`, not a rejected write.
        narrowed_by_assignment = true;
        value_ty
    } else {
        // Preserve the declared/inferred symbol type when an incompatible assignment
        // is already reported to avoid cascading return/usage diagnostics.
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    };

    if updated_ty == symbol.ty {
        return;
    }

    let updated = SymbolInfo {
        ty: updated_ty,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };

    if narrowed_by_assignment {
        // Assignment narrowing is block-scoped: written into the current frame it
        // is discarded when a branch scope pops, so `if (t === "draft-4") t = "draft-04";`
        // leaves the declared union in place for the code that follows.
        scopes.insert_current(target_name, updated);
        return;
    }

    let _ = scopes.update_visible(target_name, updated);
}
