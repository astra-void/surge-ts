use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedAssignment, ParsedExpression, ParsedMemberAssignment, ParsedThisPropertyAssignment,
    ParsedType,
};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason};

use super::super::visible_symbols;
use crate::checks::assign::check_assignment_with_symbols;
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    FlowCheck, FunctionFlowState, check_assignment_target_flow, mark_assignment_state,
};
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

pub(crate) fn check_function_assignment(
    assignment: ParsedAssignment,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let target_name = assignment.target_name.clone();
    let compound = crate::flow::is_compound_assignment(
        &assignment.target_name,
        assignment.target_span,
        &assignment.value,
    );

    let (target_blocked, value_blocked) = if flow_state.tracked_local_count() > 0 {
        (
            check_assignment_target_flow(
                &target_name,
                flow_state,
                statement_index,
                ctx,
                assignment.target_span,
            ),
            {
                let (read, read_span) = crate::flow::assignment_value_read(&assignment);
                crate::flow::check_substituting_read_flow(
                    read,
                    read_span,
                    flow_state,
                    statement_index,
                    ctx,
                )
            },
        )
    } else {
        (FlowCheck::Clear, FlowCheck::Clear)
    };

    if !target_blocked.is_blocked() && !value_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let shadowed_locally = scopes.declares_locally(&target_name);
        let assigns_empty_array = matches!(
            &assignment.value,
            ParsedExpression::ArrayLiteral { elements, .. } if elements.is_empty()
        );
        let value = assignment.value.clone();
        let value_span = assignment.value_span;
        // The value is checked in its target's context (tsc's
        // `getContextualTypeForAssignmentExpression`), which types a callback's
        // parameters; what it narrows the target to is read again without
        // that context, and reports nothing the checked value did not.
        let checked =
            check_assignment_with_symbols(assignment, &visible_symbols, shadowed_locally, ctx);
        let checkpoint = ctx.diagnostics().len();
        let inferred_value = evaluate_expression(&value, value_span, &visible_symbols, ctx);
        if checked {
            ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
        }
        if compound {
            widen_compound_assigned_symbol_type(&target_name, scopes);
        } else if !super::evolving_arrays::assign_evolving_array(
            &target_name,
            assigns_empty_array,
            &inferred_value,
            scopes,
            ctx,
        ) {
            update_assigned_symbol_type(&target_name, inferred_value, scopes);
        }
    }

    if !target_blocked.is_blocked()
        && flow_state.tracked_local_count() > 0
        && !compound
    {
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

/// tsc's `getWriteTypeOfSymbol`: a write is checked against the *setter's*
/// parameter type when an accessor pair declares two different types, while a
/// read produces the getter's. Only a non-generic declaration is answered here
/// — under type arguments the write type would have to be resolved through the
/// interface member cache, which owns that substitution.
fn accessor_write_type(
    receiver: &Type,
    property_name: &str,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let (write_ty, substitution) = {
        let (member, substitution) = declared_member(receiver, property_name, ctx)?;
        (member.write_ty.clone()?, substitution)
    };
    Some(crate::infer::types::map_parsed_type_with_substitution(
        write_ty,
        ctx,
        &substitution,
    ))
}

/// The member a write names, as its *declaration* wrote it, together with the
/// substitution its annotation resolves under — the reference's type arguments
/// bound to the declaration's own parameter names, so a generic accessor's
/// write type (`set value(next: T | string)`) resolves like any other member.
fn declared_member<'a>(
    receiver: &Type,
    property_name: &str,
    ctx: &'a CheckerContext,
) -> Option<(
    &'a surge_ts_syntax::ParsedInterfaceMember,
    crate::infer::types::TypeParameterSubstitution,
)> {
    let Type::Reference(reference) = receiver else {
        return None;
    };
    let name = reference.id.split('\u{0}').next_back()?;
    let crate::symbols::TypeDeclarationInfo::Interface(info) = ctx.lookup_type_declaration(name)?
    else {
        return None;
    };
    let member = info
        .body
        .members
        .iter()
        .find(|member| member.name == property_name)?;

    let mut substitution = crate::infer::types::TypeParameterSubstitution::new();
    for (type_parameter, argument) in info
        .body
        .type_parameters
        .iter()
        .zip(reference.arguments.iter())
    {
        substitution.set(type_parameter.name.clone(), argument.clone(), false);
    }

    Some((member, substitution))
}

/// Reports a literal tuple index outside the tuple's fixed length (TS2493) or a
/// negative one (TS2514), and answers `undefined` as the element they name —
/// tsc reports the assigned value against that, so a write to a missing element
/// carries both diagnostics.
fn tuple_index_out_of_bounds(
    receiver: &Type,
    index_type: &Type,
    span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Tuple(elements) = receiver.peeled() else {
        return None;
    };
    let Type::NumberLiteral(literal) = index_type else {
        return None;
    };
    let index = literal.value.parse::<i64>().ok()?;

    let diagnostic = if index < 0 {
        Diagnostic::ts2514(ctx.file_name.clone())
    } else if index as usize >= elements.len() {
        Diagnostic::ts2493(
            Type::Tuple(elements.clone()).name(),
            elements.len(),
            index,
            ctx.file_name.clone(),
        )
    } else {
        return None;
    };

    ctx.push(match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
    Some(Type::Undefined)
}

/// tsc checks a write's value whatever its target turns out to be
/// (`checkBinaryLikeExpression` checks both operands before the assignment).
/// With no target type there is no contextual type either; where the receiver
/// is one surge could not model rather than tsc's error type, what that
/// missing context would report is surge's gap and is suppressed.
fn check_value_without_target(
    assignment: &ParsedMemberAssignment,
    receiver: Option<&Type>,
    symbols: &crate::symbols::SymbolTable,
    ctx: &mut CheckerContext,
) {
    let degraded = !matches!(receiver, Some(Type::ErrorType));
    if degraded {
        ctx.degraded_expected_type_depth += 1;
    }
    let (value, value_span) = compound_operand(assignment).unwrap_or((&assignment.value, assignment.value_span));
    let _ = evaluate_expression(value, value_span, symbols, ctx);
    if degraded {
        ctx.degraded_expected_type_depth -= 1;
    }
}

/// The written operand of `o.p op= v`, which the parser carries as the value
/// `o.p op v`: the target's own read is the write the caller already
/// resolved, so only `v` is left to check.
fn compound_operand(assignment: &ParsedMemberAssignment) -> Option<(&ParsedExpression, Option<surge_ts_syntax::TextSpan>)> {
    let (left, left_span, right, right_span) = match &assignment.value {
        ParsedExpression::Binary { left, left_span, right, right_span, .. }
        | ParsedExpression::Logical { left, left_span, right, right_span, .. }
        | ParsedExpression::NullishCoalescing { left, left_span, right, right_span } => {
            (left, left_span, right, right_span)
        }
        _ => return None,
    };
    (*left_span == assignment.target_span && **left == assignment.target).then_some((right.as_ref(), *right_span))
}

/// tsc's `isReadonlySymbol` for the members surge records it for: a `readonly`
/// property and a getter with no setter. The write is reported and the
/// assignability check skipped, exactly as tsc does after `checkReferenceExpression`
/// returns the error type.
fn report_readonly_property_write(
    receiver: &Type,
    property_name: &str,
    span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    if !property_write_is_readonly(receiver, property_name, ctx) {
        return false;
    }

    let diagnostic = Diagnostic::ts2540(property_name, ctx.file_name.clone());
    ctx.push(match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
    true
}

/// Whether writing `receiver.property_name` is rejected because the member is
/// `readonly`. Shared with the `delete` and increment/decrement operand checks,
/// which reject the same members under their own diagnostic codes.
pub(crate) fn property_write_is_readonly(
    receiver: &Type,
    property_name: &str,
    ctx: &CheckerContext,
) -> bool {
    object_property_is_readonly(receiver, property_name)
        || declared_member(receiver, property_name, ctx).is_some_and(|(member, _)| member.readonly)
}

/// Whether the receiver's own object surface declares the member `readonly`.
/// A union is read-only in the member only when every constituent is, which is
/// what tsc's synthetic union property records.
fn object_property_is_readonly(receiver: &Type, property_name: &str) -> bool {
    match receiver.peeled() {
        Type::Object(object) => object
            .properties
            .get(property_name)
            .is_some_and(|property| property.readonly),
        Type::Union(union) => union
            .types()
            .iter()
            .all(|member| object_property_is_readonly(member, property_name)),
        _ => false,
    }
}

/// What a write through an index into a `readonly` array or tuple reaches: a
/// tuple's fixed element is a property, and any other index the read-only
/// index signature, named as tsc prints the receiver.
pub(crate) enum ReadonlyElementWrite {
    Property(String),
    IndexSignature(String),
}

pub(crate) fn readonly_element_write(receiver: &Type, index_type: &Type) -> Option<ReadonlyElementWrite> {
    let Type::Reference(reference) = receiver else {
        return None;
    };
    // The lib's `ReadonlyArray<T>` written by name is the same readonly array
    // as `readonly T[]`, which tsc also prints it as.
    let lib_readonly_array = reference.arguments.len() == 1
        && reference.id.split('\u{0}').next_back() == Some("ReadonlyArray");
    if !reference.is_readonly_array() && !lib_readonly_array {
        return None;
    }

    // Only the leading fixed elements of an open tuple are properties; the
    // rest of it is read through its index signature.
    let key = literal_index_key(index_type);
    match (receiver.peeled(), key) {
        (Type::Tuple(_), Some(key)) => return Some(ReadonlyElementWrite::Property(key)),
        (Type::OpenTuple(tuple), Some(key)) if tuple.leading_element(&key).is_some() => {
            return Some(ReadonlyElementWrite::Property(key));
        }
        _ => {}
    }
    let name = if lib_readonly_array {
        let element = &reference.arguments[0];
        match element {
            Type::Union(_) | Type::Function(_) => format!("readonly ({})[]", element.name()),
            _ => format!("readonly {}[]", element.name()),
        }
    } else {
        receiver.name()
    };
    Some(ReadonlyElementWrite::IndexSignature(name))
}

/// A write through an index into a `readonly` array or tuple. tsc reports the
/// index signature (TS2542) for an array-like receiver and the element itself
/// (TS2540) for a tuple, whose elements are properties.
fn report_readonly_element_write(
    receiver: &Type,
    index_type: &Type,
    element_span: Option<surge_ts_syntax::TextSpan>,
    access_span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    let Some(write) = readonly_element_write(receiver, index_type) else {
        return false;
    };

    // A tuple element is a property, reported on the index; an array's index
    // signature is reported on the whole access (`errorIfWritingToReadonlyIndex`).
    let (diagnostic, span) = match write {
        ReadonlyElementWrite::Property(key) => {
            (Diagnostic::ts2540(key, ctx.file_name.clone()), element_span)
        }
        ReadonlyElementWrite::IndexSignature(name) => {
            (Diagnostic::ts2542(name, ctx.file_name.clone()), access_span)
        }
    };
    ctx.push(match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
    true
}

/// The type a write through `receiver[index]` is checked against: the element
/// of an array, the element a literal index selects out of a tuple, the member
/// a literal key names, or an index signature's value type. `None` leaves the
/// write unchecked — the receiver is a shape whose element type surge cannot
/// name, and reporting against a guess would be a false positive.
fn element_write_target_type(receiver: &Type, index_type: &Type) -> Option<Type> {
    match receiver {
        Type::Array(element) => {
            is_assignable_to(index_type, &Type::Number).then(|| element.as_ref().clone())
        }
        Type::Tuple(elements) => {
            if let Some(index) = crate::infer::tuple_index_value(index_type) {
                // An out-of-range index is an error in its own right (TS2493),
                // which surge does not report yet; it is not an assignability
                // failure, so the write stays unchecked rather than being held
                // to a made-up element type.
                return elements.get(index).cloned();
            }
            is_assignable_to(index_type, &Type::Number).then(|| union_type(elements.to_vec()))
        }
        Type::Object(object) => {
            if let Some(key) = literal_index_key(index_type)
                && let Some(member) = object.get_property_access_type(&key)
            {
                return Some(member);
            }
            // A numeric key prefers the number index signature and falls back to
            // the string one; every other key can only use the string index.
            let key_is_numeric = literal_index_key(index_type)
                .map(|key| surge_ts_types::is_numeric_key(&key))
                .unwrap_or_else(|| is_assignable_to(index_type, &Type::Number));
            object.applicable_index_type(key_is_numeric).cloned()
        }
        // A nominal reference (`Array<number>`, an alias) writes like whatever
        // it names; an open tuple like the array of everything it can hold.
        Type::Reference(_) => match receiver.peeled() {
            peeled @ (Type::Array(_) | Type::Tuple(_) | Type::Object(_)) => {
                element_write_target_type(&peeled, index_type)
            }
            _ => None,
        },
        Type::OpenTuple(tuple) => {
            is_assignable_to(index_type, &Type::Number).then(|| tuple.element_union())
        }
        // A union receiver writes against the union of its members' write
        // types — tsc's synthetic union property defers to the union of its
        // write constituents (`getWriteTypeOfSymbolWithDeferredType`). Every
        // member has to answer: one that does not is a missing property, which
        // this path does not report.
        Type::Union(union) => {
            let mut members = Vec::with_capacity(union.types().len());
            for member in union.types() {
                members.push(element_write_target_type(member, index_type)?);
            }
            Some(union_type(members))
        }
        Type::Never => Some(Type::Never),
        _ => None,
    }
}

/// A property written through a union receiver whose members declare it as an
/// accessor: tsc's synthetic union property writes against the union of its
/// constituents' write types (`createUnionOrIntersectionProperty`), a setter's
/// parameter type where a member has one.
fn union_accessor_write_type(
    receiver: &Type,
    property_name: &str,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Union(union) = receiver else {
        return None;
    };
    let mut has_accessor = false;
    let mut targets = Vec::with_capacity(union.types().len());
    for member in union.types() {
        match accessor_write_type(member, property_name, ctx) {
            Some(target) => {
                has_accessor = true;
                targets.push(target);
            }
            None => targets.push(member.get_property_access_type(property_name)?),
        }
    }
    has_accessor.then(|| union_type(targets))
}

/// A literal key written through a union receiver: tsc's synthetic union
/// property writes against the union of its constituents' write types, an
/// accessor's setter type included (`createUnionOrIntersectionProperty`).
fn union_receiver_write_target_type(
    receiver: &Type,
    index_type: &Type,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Union(union) = receiver else {
        return None;
    };
    let key = literal_index_key(index_type)?;
    let mut targets = Vec::with_capacity(union.types().len());
    for member in union.types() {
        let target = accessor_write_type(member, &key, ctx)
            .or_else(|| element_write_target_type(member, index_type))?;
        targets.push(target);
    }
    Some(union_type(targets))
}

/// A key that is itself a union (`o[k]` with `k: "a" | "b"`) writes against the
/// *intersection* of what each key names — tsc's
/// `getIndexedAccessTypeOrUndefined` intersects the constituents' types under
/// `AccessFlags.Writing`, so two members of different types leave `never` and
/// no value can be written.
fn union_index_write_target_type(
    receiver: &Type,
    index_type: &Type,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Union(union) = index_type else {
        return None;
    };

    let mut targets = Vec::with_capacity(union.types().len());
    for key_type in union.types() {
        let key = literal_index_key(key_type)?;
        let target = accessor_write_type(receiver, &key, ctx)
            .or_else(|| element_write_target_type(receiver, key_type))?;
        targets.push(target);
    }

    (!targets.is_empty()).then(|| crate::infer::types::merge_intersection_members(targets))
}

/// The property name a literal index names. A numeric key indexes an object by
/// its string form, which is why `record[1]` reaches a string index signature.
fn literal_index_key(index_type: &Type) -> Option<String> {
    match index_type {
        Type::StringLiteral(value) => Some(value.clone()),
        Type::NumberLiteral(literal) => Some(literal.value.clone()),
        _ => None,
    }
}

/// `receiver[index] = value`. The written element resolves exactly as a read of
/// the same expression does; before this the whole statement was dropped, so
/// `arr[1] = "s"` and `tuple[0] = "s"` went unreported.
fn check_element_assignment(
    object: &ParsedExpression,
    object_span: Option<surge_ts_syntax::TextSpan>,
    index: &ParsedExpression,
    index_span: Option<surge_ts_syntax::TextSpan>,
    assignment: &ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);

    let InferredExpression::Known(object_type) = evaluate_expression(
        object,
        object_span.or(assignment.target_span),
        &visible_symbols,
        ctx,
    ) else {
        return;
    };

    let InferredExpression::Known(index_type) = evaluate_expression(
        index,
        index_span.or(assignment.target_span),
        &visible_symbols,
        ctx,
    ) else {
        return;
    };

    // A write is checked against the *declared* element type, not whatever the
    // enclosing branch narrowed the receiver to.
    let receiver_type = declared_reference_type(object, &visible_symbols).unwrap_or(object_type);

    let property_span = index_span.or(assignment.target_span);

    // tsc sets `AccessFlags.NoIndexSignatures` for a generic receiver, so a
    // write that lands on an index signature is TS2862 instead. Two keys do
    // *not* land on one and are excluded: a literal names a member of the
    // constraint, and a generic key (`K extends keyof T`) defers to an
    // indexed-access type — `shouldDeferIndexedAccessType` returns before the
    // index-signature lookup, which is why zod's
    // `defineLazy<T, K extends keyof T>` writes without complaint.
    if let Type::TypeParameter(type_parameter) = &receiver_type
        && matches!(index_type, Type::String | Type::Number | Type::Symbol)
    {
        let diagnostic = Diagnostic::ts2862(type_parameter.name.clone(), ctx.file_name.clone());
        ctx.push(match object_span.or(assignment.target_span) {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
        return;
    }

    if report_readonly_element_write(
        &receiver_type,
        &index_type,
        property_span,
        assignment.target_span.or(property_span),
        ctx,
    ) {
        return;
    }
    if let Some(key) = literal_index_key(&index_type)
        && report_readonly_property_write(&receiver_type, &key, property_span, ctx)
    {
        return;
    }

    // A literal index outside a tuple's fixed length is an error in its own
    // right, and the element it names is `undefined` — which is what tsc then
    // reports the assigned value against, so both diagnostics appear.
    let out_of_bounds_target =
        tuple_index_out_of_bounds(&receiver_type, &index_type, property_span, ctx);

    let index_indexes_as_number = matches!(
        index,
        ParsedExpression::Identifier { name, .. }
            if visible_symbols
                .get(name)
                .is_some_and(|symbol| matches!(symbol.kind, crate::symbols::SymbolKind::ForInNumericKey))
    );

    // A key a number-only receiver cannot answer is an implicit `any`, and a
    // write through one is the same TS7015 a read of it reports.
    if !index_indexes_as_number
        && ctx.options.no_implicit_any
        && let Type::Object(object) = receiver_type.peeled()
        && object.number_index_type.is_some()
        && object
            .applicable_index_type(is_assignable_to(&index_type, &Type::Number))
            .is_none()
        && literal_index_key(&index_type)
            .and_then(|key| object.get_property_access_type(&key))
            .is_none()
    {
        let diagnostic = Diagnostic::ts7015(ctx.file_name.clone());
        ctx.push(match property_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
        return;
    }

    let target_type = out_of_bounds_target
        .or_else(|| {
            literal_index_key(&index_type)
                .and_then(|key| accessor_write_type(&receiver_type, &key, ctx))
        })
        .or_else(|| union_receiver_write_target_type(&receiver_type, &index_type, ctx))
        .or_else(|| union_index_write_target_type(&receiver_type, &index_type, ctx))
        .or_else(|| {
            element_write_target_type(
                &receiver_type,
                // A `for…in` key over a numerically-keyed object writes through
                // the numeric index signature, as it reads through it.
                if index_indexes_as_number {
                    &Type::Number
                } else {
                    &index_type
                },
            )
        });
    let Some(target_type) = target_type else {
        return;
    };

    let assigned = check_assigned_value(&target_type, assignment, &visible_symbols, ctx);

    // A write is what a later read of the same element sees: tsc narrows an
    // element access with a literal or const-like key to the assigned type, and
    // without it `counts[key] = (counts[key] ?? 0) + 1` left the next read of
    // `counts[key]` possibly-undefined under `noUncheckedIndexedAccess`.
    if let Some(assigned) = assigned
        && let Some(key) = crate::checks::function::element_reference_key(object, index)
    {
        let _ = scopes.insert_current_narrowed(
            key,
            crate::symbols::SymbolInfo {
                ty: assigned,
                kind: crate::symbols::SymbolKind::Var,
                function_signature: None,
            },
            target_type,
        );
    }
}

/// The value half of a member write: evaluate it against the target type and
/// report a mismatch. Shared by the property and element paths.
fn check_assigned_value(
    target_type: &Type,
    assignment: &ParsedMemberAssignment,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    // tsc reports a mismatch of the whole value on the assignment's target.
    let inferred_value = crate::checks::expected::evaluate_expression_with_expected_type_anchored(
        &assignment.value,
        assignment.value_span,
        assignment.target_span,
        Some(target_type),
        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
        symbols,
        ctx,
    );

    let InferredExpression::Known(value_type) = inferred_value else {
        return None;
    };

    if value_type.is_unmodelled() || target_type.is_unmodelled() {
        return None;
    }

    if is_assignable_to(&value_type, target_type) {
        return Some(value_type);
    }

    let reported_target = crate::checks::expr::reported_relation_target(&value_type, &target_type);
    let diagnostic = crate::checks::expr::assignability_mismatch_diagnostic(
        &value_type,
        &reported_target,
        &crate::checks::expr::source_display_name(&value_type, &reported_target),
        &reported_target.name(),
        false,
        ctx.file_name.clone(),
    );
    // tsc anchors the assignment's type error on the whole assignment, which
    // starts at its target — not on the value.
    let diagnostic = match assignment.target_span.or(assignment.value_span) {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };
    ctx.push(diagnostic);
    None
}

/// `exports.foo = …` / `module.exports = …` in a JavaScript file is a CommonJS
/// export *declaration*, which tsc's binder turns into one
/// (`getAssignmentDeclarationKind`) rather than a write to an undeclared
/// `exports`. Checking it as a write reported a false TS2304 on the receiver.
fn is_commonjs_export_declaration(target: &ParsedExpression, ctx: &CheckerContext) -> bool {
    let lower = ctx.file_name.to_ascii_lowercase();
    if !(lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs"))
    {
        return false;
    }

    let mut root = target;
    loop {
        match root {
            ParsedExpression::PropertyAccess { object, .. }
            | ParsedExpression::ElementAccess { object, .. } => root = object,
            ParsedExpression::Identifier { name, .. } => {
                return name == "exports" || name == "module";
            }
            ParsedExpression::IndexAccess { object_name, .. } => {
                return object_name == "exports" || object_name == "module";
            }
            _ => return false,
        }
    }
}

/// tsc binds `fn.x = …` as a declaration of `x` on `fn` (an expando) when `fn`
/// is a function declaration or a `const` initialized with a function or arrow
/// expression (`getInitializerSymbol`). surge does not keep the initializer, so
/// any `const` holding a function counts.
fn is_expando_receiver(object: &ParsedExpression, symbols: &SymbolTable) -> bool {
    let ParsedExpression::Identifier { name, .. } = object else {
        return false;
    };
    let Some(symbol) = symbols.get(name) else {
        return false;
    };
    let callable = |ty: &Type| match ty.peeled() {
        Type::Function(_) => true,
        Type::Object(object) => object.call_signature().is_some(),
        _ => false,
    };
    match symbol.kind {
        SymbolKind::Function => true,
        // A member declared in one branch only leaves the join of the function
        // with and without it; it is the same expando receiver.
        SymbolKind::Const => match &symbol.ty {
            Type::Union(union) => union.types().iter().all(callable),
            other => callable(other),
        },
        _ => false,
    }
}

/// Records `fn.x = value` on the binding, so the reads after it see the member
/// tsc declares there: the function becomes `{ (…): R; x: typeof value }`.
fn declare_expando_member(
    object: &ParsedExpression,
    property_name: &str,
    assignment: &ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let ParsedExpression::Identifier { name, .. } = object else {
        return;
    };
    let symbols = visible_symbols(scopes);
    let InferredExpression::Known(value_type) =
        evaluate_expression(&assignment.value, assignment.value_span, &symbols, ctx)
    else {
        return;
    };
    if value_type.is_unknown() {
        return;
    }
    let Some(symbol) = scopes.resolve(name) else {
        return;
    };
    let members = match &symbol.ty {
        Type::Union(union) => union.types().to_vec(),
        other => vec![other.clone()],
    };
    let mut call_signature = None;
    let mut properties = surge_ts_types::PropertyMap::default();
    for member in &members {
        match member.peeled() {
            Type::Function(function) => {
                call_signature.get_or_insert(function);
            }
            Type::Object(object) => {
                let Some(signature) = object.call_signature() else {
                    return;
                };
                call_signature.get_or_insert(signature.clone());
                // A member only some branches declared may be absent.
                for (name, property) in object.properties.iter() {
                    properties.entry(name.clone()).or_insert_with(|| {
                        let mut property = property.clone();
                        property.optional |= members.len() > 1;
                        property
                    });
                }
            }
            _ => return,
        }
    }
    let Some(call_signature) = call_signature else {
        return;
    };
    let value_type = crate::checks::var::widen_implicit_variable_initializer_type(
        SymbolKind::Let,
        &assignment.value,
        &value_type,
        false,
    );
    properties.insert(
        property_name.into(),
        surge_ts_types::ObjectProperty::required(value_type),
    );
    let updated = SymbolInfo {
        ty: Type::Object(
            crate::metrics::alloc_object_type(properties, None).with_call_signature(call_signature),
        ),
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let _ = scopes.update_visible(name, updated);
}

pub(crate) fn check_member_assignment(
    assignment: ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    // `x[n] = v` on an evolving array writes through an `any[]` receiver.
    let evolving_receiver = ctx.auto_arrays_declared
        && super::evolving_arrays::element_write_receiver(
            &assignment.target,
            scopes.visible_symbols(),
        )
        .is_some();
    if evolving_receiver {
        ctx.evolving_array_operation_target = match &assignment.target {
            ParsedExpression::IndexAccess { object_span, .. } => *object_span,
            ParsedExpression::ElementAccess { object, .. } => match object.as_ref() {
                ParsedExpression::Identifier { span, .. } => *span,
                _ => None,
            },
            _ => None,
        };
    }
    check_member_assignment_itself(assignment, scopes, ctx);
    if evolving_receiver {
        ctx.evolving_array_operation_target = None;
    }
}

fn check_member_assignment_itself(
    assignment: ParsedMemberAssignment,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    if is_commonjs_export_declaration(&assignment.target, ctx) {
        return;
    }

    match &assignment.target {
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => {
            let object = ParsedExpression::Identifier {
                name: object_name.clone(),
                span: *object_span,
            };
            check_element_assignment(
                &object,
                *object_span,
                index,
                *index_span,
                &assignment,
                scopes,
                ctx,
            );
            return;
        }
        ParsedExpression::ElementAccess {
            object,
            object_span,
            index,
            index_span,
        } => {
            check_element_assignment(
                object,
                *object_span,
                index,
                *index_span,
                &assignment,
                scopes,
                ctx,
            );
            return;
        }
        _ => {}
    }

    let ParsedExpression::PropertyAccess {
        object,
        object_span,
        property_name,
        property_span,
        is_bracketed,
        ..
    } = &assignment.target
    else {
        return;
    };

    let visible_symbols = visible_symbols(scopes);

    // A module namespace object's members are read-only properties. The
    // member is resolved first: one the module does not export is reported as
    // a read of it would be, not as a read-only write.
    if let ParsedExpression::Identifier { name, .. } = object.as_ref()
        && ctx.is_namespace_import_binding(name)
        && !scopes.declares_locally(name)
    {
        let exported = match evaluate_expression(object, object_span.or(assignment.target_span), &visible_symbols, ctx) {
            InferredExpression::Known(namespace) => namespace.get_property_access_type(property_name).is_some(),
            _ => true,
        };
        if !exported {
            let _ = evaluate_expression(&assignment.target, assignment.target_span, &visible_symbols, ctx);
        } else if let Some(span) = property_span.or(assignment.target_span) {
            ctx.push(
                Diagnostic::ts2540(property_name, ctx.file_name.clone())
                    .with_span(convert_span(span)),
            );
        }
        check_value_without_target(&assignment, None, &visible_symbols, ctx);
        return;
    }

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
        receiver => {
            check_value_without_target(&assignment, receiver.flowing_type().as_ref(), &visible_symbols, ctx);
            return;
        }
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
    if !*is_bracketed {
        crate::checks::expr::check_member_accessibility(
            object,
            &object_type,
            property_name,
            *property_span,
            true,
            &visible_symbols,
            ctx,
        );
        // A dotted write through the index signature is the same TS4111 a
        // dotted read is.
        crate::checks::expr::emit_index_signature_access_on(
            &object_type,
            property_name,
            property_span.or(assignment.target_span),
            ctx,
        );
    }

    // A write checks against the property's *declared* type: after
    // `if (o.flag === undefined)` the read type is narrowed to `undefined`, but
    // `o.flag = true` is still an assignment to `boolean | undefined`.
    let declared_object_type = declared_reference_type(object, &visible_symbols)
        .or_else(|| declared_object_type_by_name(&object_type, ctx));
    let receiver_for_declaration = declared_object_type
        .as_ref()
        .unwrap_or(&object_type)
        .clone();
    if report_readonly_property_write(
        &receiver_for_declaration,
        property_name,
        property_span.or(assignment.target_span),
        ctx,
    ) {
        check_value_without_target(&assignment, None, &visible_symbols, ctx);
        return;
    }
    let accessor_write_type = accessor_write_type(&receiver_for_declaration, property_name, ctx)
        .or_else(|| union_accessor_write_type(&receiver_for_declaration, property_name, ctx));
    let Some(target_type) = accessor_write_type.or_else(|| {
        declared_object_type
            .as_ref()
            .and_then(|declared| declared.get_property_access_type(property_name))
            .or_else(|| {
                object_type
                    .get_property_access_type(property_name)
                    .map(|narrowed| widen_narrowed_literals(&narrowed))
            })
    }) else {
        // `decl.id = x` on `A | B` where `B` has no `id`: tsc reports the
        // member on the union. Only a union of fully modelled objects is
        // reported, so an incompletely modelled receiver stays silent.
        //
        // The union has to be the receiver's *own* form, not something it peels
        // to. A reference that peels to one may have lost members on the way —
        // `NextComponentType<…> = ComponentType<P> & { getInitialProps?… }`
        // peels to `ComponentType`'s union alone, and reporting off that made
        // `MyApp.getInitialProps = …` a false TS2339.
        let receiver = declared_object_type.as_ref().unwrap_or(&object_type);
        if is_expando_receiver(object, &visible_symbols) {
            declare_expando_member(object, property_name, &assignment, scopes, ctx);
            return;
        }
        let receiver = receiver.clone();
        if let Type::Union(union) = &receiver
            && union.types().iter().all(|member| {
                matches!(member.peeled(), Type::Object(_))
                    && !crate::checks::expr::carries_leaked_type_parameter(member, ctx)
            })
        {
            let diagnostic = crate::checks::expr::missing_property_diagnostic(
                property_name,
                &receiver,
                &visible_symbols,
                ctx,
            );
            let span = property_span.or(assignment.target_span);
            ctx.push(match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            });
        } else if !*is_bracketed
            && let InferredExpression::MissingProperty {
                property_name,
                object_type,
                span,
            } = crate::infer::infer_expression(&assignment.target, &visible_symbols, ctx)
        {
            // Any other receiver reports a missing member exactly as a read of
            // it does, gated by the same modelling checks the read applies.
            let diagnostic = match crate::checks::expr::global_this_missing_member(
                &property_name,
                &object_type,
                ctx,
            ) {
                Some(diagnostic) => diagnostic,
                None => Some(crate::checks::expr::missing_property_diagnostic(
                    &property_name,
                    &object_type,
                    &visible_symbols,
                    ctx,
                )),
            };
            if let Some(diagnostic) = diagnostic
                && let Some(span) = span.or(*property_span).or(assignment.target_span)
            {
                ctx.push(diagnostic.with_span(convert_span(span)));
            }
        }
        check_value_without_target(&assignment, Some(&receiver), &visible_symbols, ctx);
        return;
    };

    // A target surge could not fully resolve carries no contextual type for the
    // value either, so evaluating against it reports what the value could not
    // be given — an object literal's method parameter as implicit-any, say.
    // The value is still evaluated, because the narrowing the write installs
    // depends on it (`bag.patterns = []` is what makes the reads after it
    // non-optional); only what the evaluation reports is discarded. The
    // comparison below is skipped for such a target anyway.
    let target_unresolved = crate::checks::assign::type_contains_unknown(&target_type);
    let checkpoint = ctx.diagnostics().len();

    let inferred_value = crate::checks::expr::with_property_write_target(*property_span, || {
        crate::checks::expected::evaluate_expression_with_expected_type_anchored(
            &assignment.value,
            assignment.value_span,
            assignment.target_span,
            Some(&target_type),
            crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
            &visible_symbols,
            ctx,
        )
    });

    if target_unresolved {
        ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    }

    let InferredExpression::Known(value_type) = inferred_value else {
        return;
    };

    if target_unresolved || value_type.is_unmodelled() || target_type.is_unmodelled() {
        crate::checks::function::narrowing::narrow_assignment_target_in_scope(
            &assignment.target,
            &value_type,
            scopes,
        );
        return;
    }

    if !is_assignable_to(&value_type, &target_type) {
        let reported_target =
            crate::checks::expr::reported_relation_target(&value_type, &target_type);
        let diagnostic = crate::checks::expr::assignability_mismatch_diagnostic(
            &value_type,
            &reported_target,
            &crate::checks::expr::source_display_name(&value_type, &reported_target),
            &reported_target.name(),
            false,
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
    scopes: &mut ScopeStack,
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
        // A write to a member `this` does not declare is TS2339 in a `.ts`
        // file (only JS binds `this.x = …` as a declaration), reported exactly
        // where a read of it would be.
        let read = ParsedExpression::PropertyAccess {
            object: Box::new(ParsedExpression::This { span: None }),
            object_span: None,
            property_name: assignment.property_name.clone(),
            property_span: assignment.property_span,
            is_bracketed: false,
            binding_element: false,
        };
        if let InferredExpression::MissingProperty {
            property_name,
            object_type,
            span,
        } = crate::infer::infer_expression(&read, &visible_symbols, ctx)
            && let Some(span) = span.or(assignment.property_span)
        {
            let diagnostic = match crate::checks::expr::global_this_missing_member(
                &property_name,
                &object_type,
                ctx,
            ) {
                Some(diagnostic) => diagnostic,
                None => Some(crate::checks::expr::missing_property_diagnostic(
                    &property_name,
                    &object_type,
                    &visible_symbols,
                    ctx,
                )),
            };
            if let Some(diagnostic) = diagnostic {
                ctx.push(diagnostic.with_span(convert_span(span)));
            }
        }
        return;
    };

    // The write goes through the setter's accessibility
    // (`getDeclarationModifierFlagsFromSymbol` with `isWrite`).
    let this_receiver = visible_symbols
        .declared_type("this")
        .cloned()
        .unwrap_or_else(|| this_symbol.ty.clone());
    crate::checks::expr::check_member_accessibility(
        &ParsedExpression::This { span: None },
        &this_receiver,
        &assignment.property_name,
        assignment.property_span,
        true,
        &visible_symbols,
        ctx,
    );

    let constructor_may_write = ctx
        .constructor_writable_members
        .as_ref()
        .is_some_and(|members| members.contains(&assignment.property_name));
    if !constructor_may_write {
        let receiver = visible_symbols
            .declared_type("this")
            .cloned()
            .unwrap_or_else(|| this_symbol.ty.clone());
        if report_readonly_property_write(
            &receiver,
            &assignment.property_name,
            assignment.property_span,
            ctx,
        ) {
            return;
        }
    }

    // Evaluated against the member like an `o.p = …` write: the target types
    // the value contextually, an object literal elaborates into its members, and
    // a whole-value mismatch is reported on `this.<property>`.
    let target_unresolved = crate::checks::assign::type_contains_unknown(&property_type);
    let checkpoint = ctx.diagnostics().len();
    let inferred_value =
        crate::checks::expr::with_property_write_target(assignment.property_span, || {
            crate::checks::expected::evaluate_expression_with_expected_type_anchored(
                &assignment.value,
                assignment.value_span,
                assignment.target_span,
                Some(&property_type),
                crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
                &visible_symbols,
                ctx,
            )
        });
    if target_unresolved {
        ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    }

    let InferredExpression::Known(value_type) = inferred_value else {
        return;
    };

    // The write narrows `this.<property>` for the reads after it, as any
    // property reference's assignment does (`getAssignmentReducedType`).
    let target = ParsedExpression::PropertyAccess {
        object: Box::new(ParsedExpression::This { span: None }),
        object_span: None,
        property_name: assignment.property_name.clone(),
        property_span: assignment.property_span,
        is_bracketed: false,
        binding_element: false,
    };
    if target_unresolved || value_type.is_unmodelled() || property_type.is_unmodelled() {
        crate::checks::function::narrowing::narrow_assignment_target_in_scope(&target, &value_type, scopes);
        return;
    }

    if is_assignable_to(&value_type, &property_type) {
        crate::checks::function::narrowing::narrow_assignment_target_in_scope(&target, &value_type, scopes);
    } else {
        let reported_target =
            crate::checks::expr::reported_relation_target(&value_type, &property_type);
        let diagnostic = crate::checks::expr::assignability_mismatch_diagnostic(
            &value_type,
            &reported_target,
            &crate::checks::expr::source_display_name(&value_type, &reported_target),
            &reported_target.name(),
            false,
            ctx.file_name.clone(),
        );
        let diagnostic = match assignment.target_span.or(assignment.value_span) {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// tsc's `getAssignmentReducedType`: an assignment narrows a union-declared
/// binding to the declared members the value may be assigned to — `result =
/// { type: "data", data }` leaves the declared `data` member, optional `id`
/// and all, not the literal's own shape. A fresh boolean literal keeps its
/// literal; surge's `boolean` is not a `true | false` union, so that member is
/// narrowed here directly. When the members kept do not admit the value, the
/// value itself stands in (tsc falls back to the declaration; surge's
/// relation is incomplete enough that the value is the safer answer).
pub(super) fn assignment_reduced_type(declared: Option<&Type>, assigned: Type) -> Type {
    // An `any` fits every member, so tsc keeps the whole declaration; surge's
    // `any` here is mostly its own degradation of a value tsc can type, and
    // spreading it back over the declared union reports members (`undefined`)
    // the real value rules out.
    if matches!(assigned, Type::Any) {
        return assigned;
    }
    // A mapped type applied to a union (`Partial<A | B>`) is the union of its
    // applications, as tsc's instantiation distributes it.
    let Some(Type::Union(declared)) = declared.map(Type::peeled) else {
        return assigned;
    };
    let kept: Vec<Type> = declared
        .types()
        .iter()
        .filter(|member| type_maybe_assignable_to(&assigned, member))
        .map(|member| match (member, &assigned) {
            (Type::Boolean, Type::BooleanLiteral(_)) => assigned.clone(),
            _ => member.clone(),
        })
        .collect();
    if kept.is_empty() {
        return assigned;
    }
    let reduced = union_type(kept);
    if is_assignable_to(&assigned, &reduced) {
        reduced
    } else {
        assigned
    }
}

fn type_maybe_assignable_to(source: &Type, target: &Type) -> bool {
    match source {
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| member == target || assignable_with_common_properties(member, target)),
        _ => assignable_with_common_properties(source, target),
    }
}

/// Assignability with tsc's weak-type rule, which surge's relation leaves
/// out: an object whose properties are all optional accepts nothing that
/// shares none of them (`isRelatedTo`'s common-property check).
fn assignable_with_common_properties(source: &Type, target: &Type) -> bool {
    if !is_assignable_to(source, target) {
        return false;
    }
    match target.peeled() {
        Type::Object(object) if crate::checks::call::is_weak_object(&object) => {
            crate::checks::call::shares_a_property(source, &object)
        }
        _ => true,
    }
}

/// Applies the assignments evaluating `expression` performs (`if ((m = f()))`,
/// `a = b = c`) to the bindings, as an assignment statement does once checked:
/// the target takes the assigned type and is definitely assigned after it. One
/// that runs on only some paths (`c && (x = 1)`) joins the assigned type with
/// the binding's and leaves it as assigned as it was.
pub(crate) fn apply_expression_assignments(
    expression: &ParsedExpression,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let assignments = crate::flow::expression_assignments(expression);
    if assignments.is_empty() {
        return;
    }
    let certain = crate::flow::certainly_assigned_names(expression);
    // Every path assigns a target a `?:` writes on both sides, so it holds one
    // of those values: their union replaces what it held before.
    let mut joined: Vec<(&str, Vec<Type>)> = Vec::new();
    for (assignment, conditional) in &assignments {
        let ParsedExpression::Assignment {
            target_name, value, ..
        } = assignment
        else {
            continue;
        };
        if *conditional && certain.contains(&target_name.as_str()) {
            if let InferredExpression::Known(ty) =
                crate::infer::infer_expression(value, visible_symbols(scopes), ctx)
            {
                match joined.iter_mut().find(|(name, _)| name == target_name) {
                    Some((_, types)) => types.push(ty),
                    None => joined.push((target_name.as_str(), vec![ty])),
                }
            }
            continue;
        }
        apply_assignment_expression(assignment, *conditional, scopes, ctx);
    }
    for (name, types) in joined {
        let value = InferredExpression::Known(union_type(types));
        if !super::evolving_arrays::assign_evolving_array(name, false, &value, scopes, ctx) {
            update_assigned_symbol_type(name, value, scopes);
        }
    }
    if flow_state.tracked_local_count() > 0 {
        crate::flow::mark_expression_assignments(expression, flow_state);
    }
}

/// The types of the assignments a condition has run once it is known true
/// (the right of a `&&` chain), for the branch that sees it true. Applied
/// before that branch narrows by the condition, which then refines them.
pub(crate) fn apply_condition_true_assignment_types(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let certain: Vec<&ParsedExpression> = crate::flow::expression_assignments(condition)
        .into_iter()
        .filter(|(_, conditional)| !conditional)
        .map(|(assignment, _)| assignment)
        .collect();
    for assignment in crate::flow::condition_true_assignments(condition) {
        if !certain.iter().any(|seen| std::ptr::eq(*seen, assignment)) {
            apply_assignment_expression(assignment, false, scopes, ctx);
        }
    }
}

/// Definite assignment for the branch that sees `condition` true.
pub(crate) fn mark_condition_true_assignments(
    condition: &ParsedExpression,
    flow_state: &mut FunctionFlowState,
) {
    if flow_state.tracked_local_count() == 0 {
        return;
    }
    for assignment in crate::flow::condition_true_assignments(condition) {
        if let ParsedExpression::Assignment { target_name, .. } = assignment {
            mark_assignment_state(target_name, flow_state);
        }
    }
}

fn apply_assignment_expression(
    assignment: &ParsedExpression,
    conditional: bool,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let ParsedExpression::Assignment {
        target_name,
        target_span,
        value,
        ..
    } = assignment
    else {
        return;
    };
    let before = scopes.resolve(target_name).map(|symbol| symbol.ty.clone());
    if crate::flow::is_compound_assignment(target_name, *target_span, value) {
        widen_compound_assigned_symbol_type(target_name, scopes);
        return;
    }
    let inferred_value = crate::infer::infer_expression(value, visible_symbols(scopes), ctx);
    let assigns_empty_array = matches!(
        value.as_ref(),
        ParsedExpression::ArrayLiteral { elements, .. } if elements.is_empty()
    );
    if !super::evolving_arrays::assign_evolving_array(
        target_name,
        assigns_empty_array,
        &inferred_value,
        scopes,
        ctx,
    ) {
        update_assigned_symbol_type(target_name, inferred_value, scopes);
    }
    if conditional
        && let (Some(before), Some(symbol)) = (before, scopes.resolve(target_name))
        && symbol.ty != before
    {
        let joined = SymbolInfo {
            ty: union_type(vec![before, symbol.ty.clone()]),
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        scopes.insert_current(target_name.as_str(), joined);
    }
}

/// tsc's `getAssignmentReducedType` for a write surge could not relate
/// exactly: the declared union keeps the members the value may inhabit, or is
/// taken whole when none can be told apart. A binding narrowed to `null` that
/// is assigned an object (`d ??= { … }`) no longer reads as `null` afterwards.
/// A non-nullable value rules out the nullable members even where surge's
/// model of the value is too coarse to relate it to the others.
fn assignment_reduced_declared_type(
    declared: Option<&Type>,
    current: &Type,
    value: &Type,
) -> Option<Type> {
    let declared = declared?;
    if declared == current || value.is_unknown() {
        return None;
    }
    let Type::Union(union) = declared else {
        return Some(declared.clone());
    };
    let nullish = |ty: &Type| matches!(ty, Type::Null | Type::Undefined | Type::Void);
    let value_nullable = match value {
        Type::Union(members) => members.types().iter().any(nullish),
        other => nullish(other),
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| is_assignable_to(value, member) || (!value_nullable && !nullish(member)))
        .cloned()
        .collect();
    Some(if kept.is_empty() {
        declared.clone()
    } else {
        union_type(kept)
    })
}

/// tsc's `getTypeAtFlowAssignment` for a compound assignment (`x += v`): the
/// binding holds the base type of what it held before, whatever the operation
/// computes — it is never narrowed to the value.
pub(crate) fn widen_compound_assigned_symbol_type(target_name: &str, scopes: &mut ScopeStack) {
    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };
    let widened = base_type_of_literal(&symbol.ty);
    if widened == symbol.ty {
        return;
    }
    let updated = SymbolInfo {
        ty: widened,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let declared = scopes
        .visible_symbols()
        .declared_type(target_name)
        .cloned()
        .unwrap_or_else(|| symbol.ty.clone());
    scopes.insert_current_narrowed(target_name, updated, declared);
}

/// tsc's `getBaseTypeOfLiteralType`.
fn base_type_of_literal(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Reference(reference) if reference.enum_base.is_some() => {
            reference.enum_base.as_deref().cloned().unwrap_or_else(|| ty.clone())
        }
        Type::Union(union) => union_type(union.types().iter().map(base_type_of_literal).collect()),
        other => other.clone(),
    }
}

fn widen_to_declared(target_name: &str, scopes: &mut ScopeStack) {
    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };
    let Some(declared) = scopes.visible_symbols().declared_type(target_name).cloned() else {
        return;
    };
    if symbol.ty == declared {
        return;
    }
    let updated = SymbolInfo {
        ty: declared.clone(),
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    scopes.insert_current_narrowed(target_name, updated, declared);
}

pub(crate) fn update_assigned_symbol_type(
    target_name: &str,
    inferred_value: InferredExpression,
    scopes: &mut ScopeStack,
) {
    let InferredExpression::Known(value_ty) = inferred_value else {
        return;
    };

    if value_ty.is_unmodelled() {
        // Outside a loop pre-pass an unmodelled value keeps the narrowing
        // rather than guessing; inside it, the back edge must not claim the
        // binding still holds what it held on entry.
        if super::branch_assignments::in_loop_prepass() {
            widen_to_declared(target_name, scopes);
        }
        return;
    }

    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };

    // A binding whose *declaration* is a union the value inhabits re-narrows to
    // that value, whatever it is narrowed to right now. Checked up front so the
    // `undefined` arm below cannot claim an annotated binding: `let ctx:
    // undefined | Ctx = undefined; ctx = make();` is `Ctx` afterwards, not the
    // declared union again, and every later use of it was reading the union.
    // Only an *un-annotated* `let x = undefined` widens.
    let declared_union_admits_value = scopes
        .visible_symbols()
        .declared_type(target_name)
        .is_some_and(|declared| {
            matches!(declared.peeled(), Type::Union(_)) && is_assignable_to(&value_ty, declared)
        });

    let mut narrowed_by_assignment = false;
    let has_declared_type = scopes
        .visible_symbols()
        .declared_type(target_name)
        .is_some();
    let updated_ty =
        if symbol.ty == Type::Undefined && !declared_union_admits_value && !has_declared_type {
            union_type(vec![
                Type::Undefined,
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || value_ty.clone()),
            ])
        } else if symbol.ty == Type::Undefined {
            narrowed_by_assignment = true;
            assignment_reduced_type(
                scopes.visible_symbols().declared_type(target_name),
                value_ty,
            )
        } else if symbol.ty == value_ty || is_assignable_to(&value_ty, &symbol.ty) {
            // Assigning to a union-declared variable narrows it to what was
            // assigned, as tsc does: the lazy-singleton idiom
            // (`let client: Redis | null = null; … client = new Redis(); return client;`)
            // otherwise keeps reading as the full union at every later use.
            if matches!(symbol.ty.peeled(), Type::Union(_)) && !value_ty.is_unmodelled() {
                narrowed_by_assignment = true;
                let declared = scopes
                    .visible_symbols()
                    .declared_type(target_name)
                    .unwrap_or(&symbol.ty);
                assignment_reduced_type(Some(declared), value_ty)
            } else {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
            }
        } else if matches!(
            symbol.ty,
            Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_)
        ) {
            union_type(vec![
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
                value_ty,
            ])
        } else if scopes
            .visible_symbols()
            .declared_type(target_name)
            .is_some_and(|declared| {
                matches!(declared.peeled(), Type::Union(_)) && is_assignable_to(&value_ty, declared)
            })
        {
            // The binding is already narrowed (by its initializer, or by an earlier
            // assignment) to something this value does not inhabit. The assignment is
            // still legal against the *declaration*, and it re-narrows to the new
            // value — `let s: Wide = "a"; s = "c";` is `"c"`, not a rejected write.
            narrowed_by_assignment = true;
            assignment_reduced_type(
                scopes.visible_symbols().declared_type(target_name),
                value_ty,
            )
        } else if let Some(reduced) = assignment_reduced_declared_type(
            scopes.visible_symbols().declared_type(target_name),
            &symbol.ty,
            &value_ty,
        ) {
            narrowed_by_assignment = true;
            reduced
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
        // leaves the declared union in place for the code that follows. The
        // declaration rides along, or a binding captured from an enclosing
        // function (an IIFE body) would read as undeclared afterwards and the
        // next write would be checked against this narrowing.
        let declared = scopes
            .visible_symbols()
            .declared_type(target_name)
            .cloned()
            .unwrap_or_else(|| symbol.ty.clone());
        scopes.insert_current_narrowed(target_name, updated, declared);
        return;
    }

    let _ = scopes.update_visible(target_name, updated);
}
