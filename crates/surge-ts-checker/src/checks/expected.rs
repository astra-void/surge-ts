use std::collections::BTreeMap;
use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedObjectProperty, TextSpan as SyntaxTextSpan};
use surge_ts_types::{ObjectProperty, Type, is_assignable_to};

use super::expr::{evaluate_expression, source_display_name};
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::metrics::alloc_object_type;
use crate::program::{
    record_assignability_check, record_object_literal_property_check, record_program_timing,
};
use crate::spans::{choose_span, diagnostic_with_syntax_span};
use crate::symbols::SymbolTable;
use surge_ts_types::{TypeCopyReason, with_type_copy_reason};

#[derive(Clone, Copy)]
pub(crate) enum ExpectedTypeDiagnostic {
    TypeNotAssignable,
    ArgumentNotAssignable,
    SatisfiesNotAssignable,
}

pub(crate) fn evaluate_expression_with_expected_type(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    expected_type: Option<&Type>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    evaluate_expression_with_expected_type_anchored(
        expression,
        fallback_span,
        None,
        expected_type,
        expected_diagnostic,
        symbols,
        ctx,
    )
}

/// Like [`evaluate_expression_with_expected_type`], but threads a `target_span`
/// that whole-value assignability diagnostics (TS2741 missing property and the
/// top-level object mismatch) anchor on, matching tsc which points such errors at
/// the assignment target (e.g. the declaration name) rather than the value. When
/// `target_span` is `None` the behavior is identical to the unanchored entry.
pub(crate) fn evaluate_expression_with_expected_type_anchored(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_type: Option<&Type>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    // A degraded expectation carries no contextual parameter types, so any
    // callback or method written against it would be reported implicit-any for
    // a shape surge failed to model rather than one the source omits.
    if expected_type.is_some_and(expectation_is_degraded) {
        ctx.degraded_expected_type_depth += 1;
        let result = evaluate_expression_with_expected_type_inner(
            expression,
            fallback_span,
            target_span,
            Some(&Type::Unknown),
            expected_diagnostic,
            symbols,
            ctx,
        );
        ctx.degraded_expected_type_depth -= 1;
        return result;
    }
    // An intersection that lost an operand keeps the survivor but marks it open.
    // The members it could not enumerate are exactly the ones a callback would
    // have been contextually typed by — tRPC's handler options are
    // `HTTPBaseHandlerOptions & CreateContextCallback<…> & { … }`, and the
    // middle operand is a conditional surge cannot decide — so an implicit-any
    // reported inside such a literal describes the dropped operand, not the
    // source. The expectation itself is kept: the properties surge *did*
    // enumerate still type their values.
    if expected_type.is_some_and(expectation_lost_an_operand) {
        ctx.degraded_expected_type_depth += 1;
        let result = evaluate_expression_with_expected_type_inner(
            expression,
            fallback_span,
            target_span,
            expected_type,
            expected_diagnostic,
            symbols,
            ctx,
        );
        ctx.degraded_expected_type_depth -= 1;
        return result;
    }

    evaluate_expression_with_expected_type_inner(
        expression,
        fallback_span,
        target_span,
        expected_type,
        expected_diagnostic,
        symbols,
        ctx,
    )
}

/// Whether the expectation is an object an intersection merge left *open*
/// because one of its operands could not be modelled. Unlike the degradation
/// sentinel this still carries real properties, so it stays the expectation and
/// only the implicit-any reporting is suppressed.
fn expectation_lost_an_operand(expected_type: &Type) -> bool {
    match expected_type {
        Type::Object(object) => object.synthetic_open_index,
        Type::Reference(_) => match expected_type.peeled() {
            Type::Object(object) => object.synthetic_open_index,
            _ => false,
        },
        _ => false,
    }
}

/// Whether the expectation is the degradation sentinel, directly or as a member
/// of a union.
///
/// A union carrying the sentinel is not a narrower expectation than the sentinel
/// alone — it is the same "surge does not know", with one shape it happened to
/// resolve. Contextual typing must not pick that shape: merging zod's two
/// `tuple` overloads yields `unknown | []`, and selecting the empty tuple made
/// every `z.tuple([a, b])` report `Type '[A, B]' is not assignable to type '[]'`.
/// A *written* `unknown` is [`Type::GenuineUnknown`], so a real annotation never
/// lands here.
fn expectation_is_degraded(expected_type: &Type) -> bool {
    match expected_type {
        Type::Unknown | Type::TypeParameter(_) => true,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Unknown | Type::TypeParameter(_))),
        _ => false,
    }
}

fn evaluate_expression_with_expected_type_inner(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_type: Option<&Type>,
    _expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(expected_type) = expected_type else {
        return evaluate_expression(expression, fallback_span, symbols, ctx);
    };

    // `new C(...)` contextually typed by a generic instance reference (e.g.
    // `Promise<void>`) lets the constructor infer its type arguments. Route it
    // through `check_new_like` with the (un-peeled) expected reference so the
    // executor's callback parameters are typed.
    if let ParsedExpression::New {
        callee,
        callee_span,
        span,
        type_arguments,
        arguments,
    } = expression
    {
        return match super::call::check_new_like(
            callee,
            *callee_span,
            *span,
            type_arguments,
            arguments,
            Some(expected_type),
            symbols,
            ctx,
        ) {
            Some(result_type) => InferredExpression::Known(result_type),
            None => InferredExpression::Unknown,
        };
    }

    // A generic call whose type parameter occurs only in the return type
    // (`const c: Ctor<MyZ> = make("x", (inst) => …)`) can infer it from the
    // contextual type alone. Route the call through the expected-type entry with
    // the *un-peeled* expected type, so the inference matches type argument for
    // type argument against the declared return reference.
    if let ParsedExpression::Call {
        callee_name,
        callee_span,
        type_arguments,
        arguments,
    } = expression
    {
        return match super::call::check_call_like_with_expected_type(
            callee_name,
            *callee_span,
            None,
            type_arguments,
            arguments,
            Some(expected_type),
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        };
    }

    // A generic expected type (`Props`, `Box<T>`, …) is a nominal
    // `Type::Reference`; peel it to its structural shape so the contextual-typing
    // dispatch below (function/tuple/array/object/union) sees the real form
    // instead of falling through to context-free evaluation.
    let peeled_expected;
    let expected_type = match expected_type {
        Type::Reference(reference) => {
            peeled_expected = reference.resolve().peeled();
            &peeled_expected
        }
        other => other,
    };

    if let (Type::Function(expected_function_type), ParsedExpression::ArrowFunction(arrow)) =
        (expected_type, expression)
    {
        let function_type = crate::checks::function::check_arrow_function_expression_anchored(
            with_type_copy_reason(TypeCopyReason::ExpectedType, || arrow.as_ref().clone()),
            Some(expected_function_type),
            target_span,
            symbols,
            ctx,
        );
        return InferredExpression::Known(Type::Function(function_type));
    }

    // A callable object expected type — an interface carrying a call signature, such
    // as React's `ForwardRefRenderFunction` — contextually types an arrow the same
    // way a bare function type does, so its parameters are not implicit-any.
    if let (Type::Object(expected_object), ParsedExpression::ArrowFunction(arrow)) =
        (expected_type, expression)
    {
        if let Some(call_signature) = expected_object.call_signature() {
            let function_type = crate::checks::function::check_arrow_function_expression_anchored(
                with_type_copy_reason(TypeCopyReason::ExpectedType, || arrow.as_ref().clone()),
                Some(call_signature),
                target_span,
                symbols,
                ctx,
            );
            return InferredExpression::Known(Type::Function(function_type));
        }
    }

    // A function contextually typed by a union (`Hook | Hook[]`, the shape hook
    // options take) draws its signature from the union's single callable member,
    // mirroring tsc's `getContextualSignature`. Without it the arrow is evaluated
    // context-free and its parameters become implicit any (false TS7006). A union
    // with several callable members stays context-free: picking one there needs
    // signature matching and guessing wrong types the parameters as the wrong shape.
    if let (Type::Union(union), ParsedExpression::ArrowFunction(_)) = (expected_type, expression) {
        let callable: Vec<&Type> = union
            .types()
            .iter()
            .filter(|member| is_contextual_callable(member))
            .collect();
        if let [member] = callable.as_slice() {
            let member = with_type_copy_reason(TypeCopyReason::ExpectedType, || (*member).clone());
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(&member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }
        // Several callable members of the same arity. tsc never sees this
        // union: it resolves the overload first and contextually types the
        // arrow from the one it picked. surge merges overloads into a single
        // permissive signature instead, so the union is its own artifact and an
        // implicit-any report here would describe that gap rather than the
        // source (`ws.addEventListener("close", (event) => …)`, whose merged
        // listener slot holds both the generic overload's callback and
        // `EventListenerOrEventListenerObject`). Members of *differing* arity
        // are a genuinely ambiguous union, which tsc also refuses to type — the
        // implicit-any there is real, so it still reports.
        if callable.len() > 1 && callable_members_share_arity(&callable) {
            ctx.degraded_expected_type_depth += 1;
            let result = evaluate_expression(expression, fallback_span, symbols, ctx);
            ctx.degraded_expected_type_depth -= 1;
            return result;
        }
    }

    if let ParsedExpression::ConstAssertion {
        expression: inner, ..
    } = expression
    {
        return evaluate_expression_with_expected_type_anchored(
            inner,
            fallback_span,
            target_span,
            Some(expected_type),
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    // `fallback ?? (…)` and `fallback || (…)` produce the whole expression's
    // value from either operand, so tsc hands the contextual type to the right
    // one just as it does to a ternary's branches. Without this an arrow written
    // as the default (`opts.onSuccess ?? ((options) => …)`) is evaluated
    // context-free and its parameters become implicit any (false TS7006). The
    // left operand keeps driving its own narrowing and stays context-free.
    if let ParsedExpression::NullishCoalescing {
        left,
        left_span,
        right,
        right_span,
    } = expression
    {
        let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
        let right_result = evaluate_expression_with_expected_type_anchored(
            right,
            right_span.or(fallback_span),
            target_span,
            Some(expected_type),
            _expected_diagnostic,
            symbols,
            ctx,
        );
        return join_nullish_coalescing(left_result, right_result);
    }

    if let ParsedExpression::Logical {
        left,
        left_span,
        operator: operator @ surge_ts_syntax::ParsedLogicalOperator::Or,
        right,
        right_span,
        ..
    } = expression
    {
        let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
        let right_result = evaluate_expression_with_expected_type_anchored(
            right,
            right_span.or(fallback_span),
            target_span,
            Some(expected_type),
            _expected_diagnostic,
            symbols,
            ctx,
        );
        return crate::checks::ops::evaluate_logical_expression(
            *operator,
            left_result,
            right_result,
        );
    }

    if matches!(expression, ParsedExpression::Conditional { .. }) {
        return evaluate_conditional_expression_with_expected_type(
            expression,
            fallback_span,
            expected_type,
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    if let (Type::Tuple(expected_elements), ParsedExpression::ArrayLiteral { elements, span }) =
        (expected_type, expression)
    {
        return evaluate_tuple_literal_with_expected_type(
            elements,
            expected_elements,
            choose_span(*span, fallback_span),
            symbols,
            ctx,
        );
    }

    if let (Type::Array(expected_element_type), ParsedExpression::ArrayLiteral { elements, span }) =
        (expected_type, expression)
    {
        return evaluate_array_literal_with_expected_type(
            elements,
            expected_element_type,
            choose_span(*span, fallback_span),
            symbols,
            ctx,
        );
    }

    // An object literal against a tuple target is checked against the tuple's
    // apparent members (indices, `length`, the array methods): tsc reports the
    // first property that is none of them as excess, at the property, rather
    // than the whole literal as unassignable.
    if let (Type::Tuple(_), ParsedExpression::ObjectLiteral { properties, span }) =
        (expected_type, expression)
        && let Some(property) = properties.iter().find(|property| {
            !property.is_spread
                && expected_type
                    .get_property_access_type(&property.name)
                    .is_none()
        })
    {
        let diagnostic =
            Diagnostic::ts2353(&property.name, &expected_type.name(), ctx.file_name.clone());
        ctx.push(diagnostic_with_syntax_span(
            diagnostic,
            choose_span(
                property.name_span,
                choose_span(property.span, choose_span(*span, fallback_span)),
            ),
        ));
        return InferredExpression::Unknown;
    }

    if let (
        Type::Object(expected_object_type),
        ParsedExpression::ObjectLiteral { properties, span },
    ) = (expected_type, expression)
    {
        return evaluate_object_literal_with_expected_type(
            properties,
            expected_object_type,
            choose_span(*span, fallback_span),
            target_span,
            _expected_diagnostic,
            symbols,
            ctx,
        );
    }

    // An object literal against a union target (an overload group's merged
    // parameter, `A | B`) is contextually typed by the member that declares the
    // most of the written properties, mirroring how tsc picks the overload the
    // argument fits. Without it the literal is evaluated context-free and its
    // method/callback parameters lose their types (false TS7006).
    // An array literal against a union target takes the union's lone array/tuple
    // member as its contextual type, so the elements are checked against the
    // element type rather than widened context-free (`items: [{ type: "string" }]`
    // against `_JSONSchema | _JSONSchema[]` widened the literal's `type` to
    // `string` and then rejected it).
    if let (Type::Union(union), ParsedExpression::ArrayLiteral { .. }) = (expected_type, expression)
    {
        // A `readonly T[]` member contextually types the literal exactly like
        // `T[]` does; the modifier only matters for the assignability test.
        let members: Vec<Type> = union.types().iter().map(mutable_sequence_shape).collect();
        let mut array_members = members
            .iter()
            .filter(|member| matches!(member, Type::Array(_) | Type::Tuple(_)));
        if let (Some(member), None) = (array_members.next(), array_members.next()) {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }
        // Several array/tuple members: the literal itself says which one it is,
        // the way a discriminated object literal does. `match<['+', number,
        // number] | ['-', number]>(['-', 2])` has to reach the `'-'` member, and
        // `[{ type: 'b', s: '2' }]` against `A[] | B[]` has to reach `B[]`, or
        // the literal is evaluated context-free and widens.
        if let ParsedExpression::ArrayLiteral { elements, .. } = expression
            && let Some(member) = sole_matching_sequence_member(&members, elements, symbols, ctx)
        {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(&member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }
    }

    if let (Type::Union(union), ParsedExpression::ObjectLiteral { properties, .. }) =
        (expected_type, expression)
    {
        let written: Vec<&str> = properties
            .iter()
            .filter(|property| !property.is_spread)
            .map(|property| property.name.as_str())
            .collect();
        // Only an unambiguous match is used: one member must declare every
        // written property and cover strictly more of them than any other. A
        // discriminated union (every member carries `code`, `message`, …) ties
        // and stays context-free, since picking a member there needs discriminant
        // matching and guessing wrong reports the literal against the wrong one.
        let mut matching = union.types().iter().filter(|member| {
            written
                .iter()
                .any(|name| member.get_property_access_type(name).is_some())
        });
        // The sole match must also *declare* one of the written properties: a
        // string index signature answers every name, so a member that only
        // index-accesses them models nothing about the literal and typing it
        // against that member reports the member's own required properties as
        // missing.
        let unambiguous = match (matching.next(), matching.next()) {
            (Some(member), None)
                if !written.is_empty()
                    && written
                        .iter()
                        .any(|name| member_declares_property(member, name)) =>
            {
                Some(member)
            }
            _ => None,
        };
        if let Some(member) = unambiguous {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }

        // Several members declare the written properties, and the one the literal
        // belongs to may only be visible *below* the top level:
        // `{ value: A[] } | { value: B[] }` ties on names, and what decides it is
        // a literal inside the nested array. Typing the literal once against the
        // per-property union — which is the contextual type tsc uses here — gives
        // each nested literal the context it needs, and the result then picks its
        // member. Two passes, not one per candidate.
        if union_members_are_all_objects(union)
            && let Some(member) = union_member_for_object_literal(
                union.types(),
                expression,
                &written,
                fallback_span,
                target_span,
                _expected_diagnostic,
                symbols,
                ctx,
            )
        {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(&member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }

        // Several members declare the written properties (an overload group's
        // merged parameter, where every member extends the same base). tsc types
        // the literal by the union of each property across those members, so
        // that is tried first: each property under the union of what the
        // candidates declare for it, accepted when the result fits the union.
        // Failing that, surge cannot pick a member and evaluates context-free —
        // but an implicit-any report there would describe that gap, not the
        // source.
        if union_members_are_all_objects(union) {
            if let Some(result) = object_literal_under_property_unions(
                expected_type,
                union.types(),
                properties,
                fallback_span,
                target_span,
                _expected_diagnostic,
                symbols,
                ctx,
            ) {
                return result;
            }
            ctx.degraded_expected_type_depth += 1;
            let result = evaluate_expression(expression, fallback_span, symbols, ctx);
            ctx.degraded_expected_type_depth -= 1;
            return result;
        }
    }

    // Contextual typing through a union: when the expected type is a union whose
    // only non-nullish member is a single concrete type (e.g. `{ ... } | null`,
    // mapped here to `{ ... } | undefined`), use that member as the contextual
    // type so an object/array literal's property values are evaluated with the
    // expected element types rather than context-free (which would widen
    // member-access values like `res.status` toward `unknown`).
    if let Type::Union(union) = expected_type {
        let mut non_nullish = union
            .types()
            .iter()
            .filter(|member| !matches!(member, Type::Undefined | Type::Void));
        if let (Some(member), None) = (non_nullish.next(), non_nullish.next()) {
            return evaluate_expression_with_expected_type_anchored(
                expression,
                fallback_span,
                target_span,
                Some(member),
                _expected_diagnostic,
                symbols,
                ctx,
            );
        }
    }

    evaluate_expression(expression, fallback_span, symbols, ctx)
}

/// Whether `member` declares `name` as a real property.
///
/// A string index signature (and the `Object.prototype` member fallback) answers
/// every name, so `get_property_access_type` cannot tell the member that models
/// the written property from one that merely permits it. Picking the latter
/// types the literal against the wrong shape — e.g. an object literal against
/// `ReadableStream | ... | Record<string, any>` was reported as missing
/// `ReadableStream`'s members once a `.d.ts` merge gave that interface a
/// degraded index signature.
/// The union member an object literal belongs to, when several members declare
/// everything it writes. Without it the literal is evaluated context-free, its
/// *nested* literals widen, and every member then rejects it.
///
/// Two signals, cheapest first. A property written as a primitive literal has its
/// type without any evaluation, and in a discriminated union it alone decides.
/// Only a literal with no such property — `{ value: [ … ] }`, whose discriminator
/// lives inside the array — pays for one real evaluation of that one property
/// against the union of what the candidates declare for it.
///
/// Trying each candidate instead was measured and rejected: 5 false positives on
/// tRPC and tanstack-query never finishing.
fn union_member_for_object_literal(
    members: &[Type],
    expression: &ParsedExpression,
    written: &[&str],
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if written.is_empty() {
        return None;
    }
    let candidates: Vec<&Type> = members
        .iter()
        .filter(|member| {
            written
                .iter()
                .all(|name| member_declares_property(member, name))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // Typing a literal against a *wide* member is the expensive half of this, not
    // the probe: with no cap at all, and even with every probe removed, the
    // tanstack-query aggregate goes from 4 s to not finishing in five minutes on
    // its query-option unions. At 20 own properties it is 4 s again, and 40 is
    // already too high. The cap is a cost bound, so it is deliberately about the
    // candidate's size rather than the union's arity.
    const MAX_CANDIDATE_PROPERTIES: usize = 20;
    if candidates.iter().any(|member| {
        matches!(member.peeled(), Type::Object(object) if object.properties.len() > MAX_CANDIDATE_PROPERTIES)
    }) {
        return None;
    }

    let mut narrowed: Vec<&Type> = candidates.clone();
    // One member declaring *every* written property, where several declare some,
    // is already the decision — that is the same evidence the single-match check
    // above uses, read the strict way round.
    let mut decided = candidates.len() == 1 && members.len() > 1;

    // Discriminants first, and for free: a property written as a primitive
    // literal has its type without any evaluation, and in a discriminated union
    // it alone decides. Only if that leaves the set ambiguous is anything typed.
    for property in properties_of(expression) {
        if property.is_spread {
            continue;
        }
        let Some(value) = written_literal_value(&property.value) else {
            continue;
        };
        narrowed = match narrow_by_property(&narrowed, property.name.as_str(), &value) {
            Some(next) => {
                decided = true;
                next
            }
            None => narrowed,
        };
        if narrowed.len() == 1 {
            return Some(narrowed[0].clone());
        }
    }

    // No discriminant settled it. One property may still, but typing it costs a
    // real evaluation, so this is bounded to the single-property literal — the
    // `{ value: [ … ] }` shape whose discriminator lives inside a nested array —
    // and cannot nest.
    const MAX_PROBE_CANDIDATES: usize = 4;
    if !decided
        && written.len() == 1
        && narrowed.len() <= MAX_PROBE_CANDIDATES
        && ctx.union_member_probe_depth == 0
        && let Some(property) = properties_of(expression)
            .iter()
            .find(|property| property.name.as_str() == written[0])
    {
        let per_property: Vec<Type> = narrowed
            .iter()
            .filter_map(|member| member.get_property_access_type(written[0]))
            .collect();
        if per_property.len() == narrowed.len() {
            let diagnostics_before = ctx.diagnostics().len();
            ctx.union_member_probe_depth += 1;
            let evaluated = evaluate_expression_with_expected_type_anchored(
                &property.value,
                property.value_span.or(fallback_span),
                target_span,
                Some(&surge_ts_types::union_type(per_property)),
                expected_diagnostic,
                symbols,
                ctx,
            );
            ctx.union_member_probe_depth -= 1;
            ctx.truncate_diagnostics(diagnostics_before);
            if let crate::infer::InferredExpression::Known(ty) = evaluated
                && !ty.is_unknown()
                && let Some(next) = narrow_by_property(&narrowed, written[0], &ty)
            {
                decided = true;
                narrowed = next;
            }
        }
    }

    if !decided {
        return None;
    }

    let mut accepting = narrowed.into_iter();
    match (accepting.next(), accepting.next()) {
        (Some(member), None) => Some(member.clone()),
        _ => None,
    }
}

/// An object literal typed the way tsc types it against a union none of whose
/// members can be singled out: every property is evaluated under the union of
/// what the candidate members declare for it, and the literal is accepted only
/// when the shape that produces fits the union. `None` leaves the existing
/// context-free path to report; nothing is emitted here.
#[allow(clippy::too_many_arguments)]
fn object_literal_under_property_unions(
    expected_type: &Type,
    members: &[Type],
    properties: &[ParsedObjectProperty],
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<InferredExpression> {
    if properties.is_empty() || properties.iter().any(|property| property.is_spread) {
        return None;
    }
    let candidates: Vec<&Type> = members
        .iter()
        .filter(|member| {
            properties
                .iter()
                .all(|property| member_declares_property(member, &property.name))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let diagnostics_before = ctx.diagnostics().len();
    let mut inferred = surge_ts_types::PropertyMap::default();
    for property in properties {
        let per_property: Vec<Type> = candidates
            .iter()
            .filter_map(|member| member.get_property_access_type(&property.name))
            .collect();
        let evaluated = evaluate_expression_with_expected_type_anchored(
            &property.value,
            property.value_span.or(fallback_span),
            target_span,
            Some(&surge_ts_types::union_type(per_property)),
            expected_diagnostic,
            symbols,
            ctx,
        );
        let InferredExpression::Known(ty) = evaluated else {
            ctx.truncate_diagnostics(diagnostics_before);
            return None;
        };
        if ty.is_unknown() {
            ctx.truncate_diagnostics(diagnostics_before);
            return None;
        }
        inferred.insert(
            property.name.as_str().into(),
            surge_ts_types::ObjectProperty::required(ty),
        );
    }
    let literal = Type::Object(alloc_object_type(inferred, None));
    if ctx.diagnostics().len() != diagnostics_before || !is_assignable_to(&literal, expected_type) {
        ctx.truncate_diagnostics(diagnostics_before);
        return None;
    }
    Some(InferredExpression::Known(literal))
}

/// A property written as a primitive literal, as the type that literal has. The
/// discriminant of a discriminated union is always one of these, and reading it
/// costs no evaluation.
fn written_literal_value(expression: &ParsedExpression) -> Option<Type> {
    match expression {
        ParsedExpression::StringLiteral(value) => Some(Type::StringLiteral(value.clone())),
        ParsedExpression::BooleanLiteral(value) => Some(Type::BooleanLiteral(*value)),
        ParsedExpression::NumberLiteral(value) => {
            Some(Type::NumberLiteral(surge_ts_types::NumberLiteralType {
                value: value.clone(),
            }))
        }
        _ => None,
    }
}

/// The candidates whose declared `name` accepts `value`, or `None` when that
/// rules out everything or nothing — neither is evidence.
fn narrow_by_property<'a>(
    candidates: &[&'a Type],
    name: &str,
    value: &Type,
) -> Option<Vec<&'a Type>> {
    let accepting: Vec<&Type> = candidates
        .iter()
        .copied()
        .filter(|member| {
            member
                .get_property_access_type(name)
                .is_some_and(|declared| surge_ts_types::is_assignable_to(value, &declared))
        })
        .collect();
    (!accepting.is_empty() && accepting.len() < candidates.len()).then_some(accepting)
}

/// An object literal's own properties, or an empty slice for anything else.
fn properties_of(expression: &ParsedExpression) -> &[ParsedObjectProperty] {
    match expression {
        ParsedExpression::ObjectLiteral { properties, .. } => properties,
        _ => &[],
    }
}

fn member_declares_property(member: &Type, name: &str) -> bool {
    matches!(member.peeled(), Type::Object(object) if object.properties.contains_key(name))
}

/// Whether every member of a union is an object type — the shape where a
/// written object literal is genuinely contextually typed by tsc even though
/// surge cannot pick a single member to check against.
fn union_members_are_all_objects(union: &surge_ts_types::UnionType) -> bool {
    let mut object_members = 0usize;
    for member in union.types() {
        // An optional parameter contributes `undefined`; it is not a shape the
        // literal could be typed by, so it does not disqualify the union.
        if matches!(member, Type::Undefined | Type::Void) {
            continue;
        }
        if !matches!(member.peeled(), Type::Object(_)) {
            return false;
        }
        object_members += 1;
    }
    object_members >= 2
}

/// The `??` result rule, mirroring the context-free arm in
/// `checks::expr::evaluate`: the left operand contributes only its non-nullish
/// part, and either side failing to resolve degrades the whole expression.
fn join_nullish_coalescing(
    left_result: InferredExpression,
    right_result: InferredExpression,
) -> InferredExpression {
    match (left_result, right_result) {
        (InferredExpression::Known(left_type), InferredExpression::Known(right_type)) => {
            if left_type == Type::Any || left_type.is_unknown() {
                InferredExpression::Known(left_type)
            } else if left_type == Type::Undefined {
                InferredExpression::Known(right_type)
            } else {
                InferredExpression::Known(surge_ts_types::union_type(vec![
                    surge_ts_types::remove_nullish(&left_type),
                    right_type,
                ]))
            }
        }
        _ => InferredExpression::Unknown,
    }
}

/// Whether every callable member takes the same number of parameters.
fn callable_members_share_arity(members: &[&Type]) -> bool {
    let mut arity = None;
    for member in members {
        let Some(signature) = contextual_call_signature(member) else {
            return false;
        };
        let count = signature.parameters().len();
        match arity {
            None => arity = Some(count),
            Some(existing) if existing == count => {}
            Some(_) => return false,
        }
    }
    arity.is_some()
}

/// The call signature a union member contributes to contextual typing.
fn contextual_call_signature(ty: &Type) -> Option<surge_ts_types::FunctionType> {
    let peeled = ty.peeled();
    match peeled {
        Type::Function(function) => Some(function),
        Type::Object(object) => object.call_signature().cloned(),
        _ => None,
    }
}

fn is_contextual_callable(ty: &Type) -> bool {
    let peeled;
    let ty = match ty {
        Type::Reference(reference) => {
            peeled = reference.resolve().peeled();
            &peeled
        }
        other => other,
    };
    match ty {
        Type::Function(_) => true,
        Type::Object(object) => object.call_signature().is_some(),
        _ => false,
    }
}

fn evaluate_array_literal_with_expected_type(
    elements: &[surge_ts_syntax::ParsedArrayElement],
    expected_element_type: &Type,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    for element in elements {
        let inferred_element = evaluate_expression_with_expected_type(
            &element.expression,
            element.span,
            Some(expected_element_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_element {
            InferredExpression::Known(actual_type) => {
                if actual_type.is_unknown()
                    || crate::checks::call::is_open_instantiation(&actual_type)
                {
                    continue;
                }

                if !is_assignable_to(&actual_type, expected_element_type) {
                    let actual_type_name = actual_type.name();
                    let expected_type_name = expected_element_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(element.span, fallback_span),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
        }
    }

    InferredExpression::Known(Type::Array(Box::new(with_type_copy_reason(
        TypeCopyReason::ExpectedType,
        || expected_element_type.clone(),
    ))))
}

fn mutable_sequence_shape(member: &Type) -> Type {
    // `ReadonlyArray<T>` spelled as a lib reference resolves to the readonly
    // wrapper, whose own resolution is the mutable shape; `peeled` follows
    // the whole chain.
    match member.peeled() {
        sequence @ (Type::Array(_) | Type::Tuple(_)) => sequence,
        _ => member.clone(),
    }
}

/// The one array-or-tuple member of a union target an array literal can be: a
/// tuple of the same arity whose slots all accept the written elements, or an
/// array whose element type does. Element types are read *unwidened* — the
/// literal `'b'` is what tells `B[]` from `A[]`, and `['-', 2]` from
/// `['++', number]`. `None` when the literal is ambiguous or fits nothing, so
/// the caller keeps its context-free behavior.
fn sole_matching_sequence_member(
    members: &[Type],
    elements: &[surge_ts_syntax::ParsedArrayElement],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let candidates: Vec<&Type> = members
        .iter()
        .filter(|member| match member {
            Type::Tuple(slots) => slots.len() == elements.len(),
            Type::Array(_) => true,
            _ => false,
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // `[]` writes nothing to tell members apart by, so it is the empty tuple
    // when the union declares one and otherwise the lone array member;
    // evaluated context-free it would be `any[]`, which fits no tuple.
    if elements.is_empty() {
        let mut empty_tuples = candidates
            .iter()
            .filter(|member| matches!(member, Type::Tuple(_)));
        return match (empty_tuples.next(), empty_tuples.next()) {
            (Some(member), None) => Some((*member).clone()),
            (None, _) if candidates.len() == 1 => Some(candidates[0].clone()),
            _ => None,
        };
    }

    let diagnostics_before = ctx.diagnostics().len();
    let mut written = Vec::with_capacity(elements.len());
    for element in elements {
        match crate::infer::infer_expression(&element.expression, symbols, ctx) {
            crate::infer::InferredExpression::Known(ty) if !ty.is_unknown() => written.push(ty),
            _ => {
                ctx.truncate_diagnostics(diagnostics_before);
                return None;
            }
        }
    }
    ctx.truncate_diagnostics(diagnostics_before);

    let mut fitting = candidates.iter().filter(|member| match member {
        Type::Tuple(slots) => slots
            .iter()
            .zip(written.iter())
            .all(|(slot, value)| surge_ts_types::is_assignable_to(value, slot)),
        Type::Array(element) => written
            .iter()
            .all(|value| surge_ts_types::is_assignable_to(value, element)),
        _ => false,
    });
    match (fitting.next(), fitting.next()) {
        (Some(member), None) => return Some((*member).clone()),
        // Several tuples of the literal's arity fit (`['line']` against
        // `['list'] | ['line'] | [string] | …`): tsc types each element by the
        // union of the candidates' slots, which keeps the literal a tuple of
        // literals that then fits the union target. Arrays among the fits
        // keep the literal ambiguous.
        (Some(first), Some(second)) => {
            let fitting_tuples: Vec<&[Type]> = std::iter::once(first)
                .chain(std::iter::once(second))
                .chain(fitting)
                .map(|member| match member {
                    Type::Tuple(slots) => Some(slots.as_slice()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let slots = (0..elements.len())
                .map(|index| {
                    surge_ts_types::union_type(
                        fitting_tuples
                            .iter()
                            .map(|slots| slots[index].clone())
                            .collect(),
                    )
                })
                .collect();
            return Some(Type::Tuple(slots));
        }
        (None, _) => {}
    }

    // Nothing fit, which does not mean nothing belongs: an element that is
    // itself an object literal was inferred context-free above, so its own
    // nested literals widened and it matched no slot. Fall back to the test the
    // object-literal path uses one level up — the candidate whose slot *declares*
    // every property the element writes. That is what separates a datadog-shaped
    // `{ response_format, queries }` request from a `{ q }` one.
    let mut declaring = candidates.iter().filter(|member| {
        elements.iter().enumerate().all(|(index, element)| {
            let slot = match member {
                Type::Tuple(slots) => slots.get(index),
                Type::Array(element_type) => Some(element_type.as_ref()),
                _ => None,
            };
            match (slot, &element.expression) {
                (Some(slot), ParsedExpression::ObjectLiteral { properties, .. }) => properties
                    .iter()
                    .filter(|property| !property.is_spread)
                    .all(|property| member_declares_property(slot, property.name.as_str())),
                _ => false,
            }
        })
    });
    match (declaring.next(), declaring.next()) {
        (Some(member), None) => Some((*member).clone()),
        _ => None,
    }
}

/// Every element's own widened type, with an element that does not resolve
/// standing in as `any` — tsc's error type, which it renders the same way
/// (`Type '[string, number, any]' is not assignable to type '[string, number]'`).
/// The inference is a probe, so its diagnostics are discarded: the unresolved
/// element is reported by the pass that owns it, not by this message.
fn literal_element_types(
    elements: &[surge_ts_syntax::ParsedArrayElement],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Vec<Type> {
    let diagnostics_before = ctx.diagnostics().len();
    let mut element_types = Vec::with_capacity(elements.len());
    for element in elements {
        let element_type = match crate::infer::infer_expression(&element.expression, symbols, ctx) {
            crate::infer::InferredExpression::Known(ty) if !ty.is_unknown() => {
                crate::checks::expr::widen_type(&ty)
            }
            _ => Type::Any,
        };
        element_types.push(element_type);
    }
    ctx.truncate_diagnostics(diagnostics_before);
    element_types
}

fn evaluate_tuple_literal_with_expected_type(
    elements: &[surge_ts_syntax::ParsedArrayElement],
    expected_elements: &[Type],
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    for (index, element) in elements.iter().enumerate() {
        if index >= expected_elements.len() {
            // The literal is longer than the tuple allows. tsc names the source by
            // its own widened element types (`Type '[number]' is not assignable to
            // type '[]'`); the hardcoded `unknown[]` this used to print named
            // neither side truthfully.
            let source_type_name =
                Type::Tuple(literal_element_types(elements, symbols, ctx)).name();
            let target_type_name = Type::Tuple(expected_elements.to_vec()).name();
            let diagnostic =
                Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                choose_span(element.span, fallback_span),
            ));
            return InferredExpression::Unknown;
        }

        let expected_element_type = &expected_elements[index];
        let inferred_element = evaluate_expression_with_expected_type(
            &element.expression,
            element.span,
            Some(expected_element_type),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );

        match inferred_element {
            InferredExpression::Known(actual_type) => {
                if actual_type.is_unknown()
                    || crate::checks::call::is_open_instantiation(&actual_type)
                {
                    continue;
                }

                if !is_assignable_to(&actual_type, expected_element_type) {
                    let actual_type_name = actual_type.name();
                    let expected_type_name = expected_element_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(element.span, fallback_span),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
        }
    }

    // A literal may stop short of trailing slots that accept `undefined` —
    // how an optional element (`[string[], Opts?]`) is represented.
    let trailing_optional = expected_elements[elements.len().min(expected_elements.len())..]
        .iter()
        .all(|slot| is_assignable_to(&Type::Undefined, slot));
    if elements.len() != expected_elements.len() && !trailing_optional {
        let source_type_name = Type::Array(Box::new(Type::Unknown)).name();
        let target_type_name = Type::Tuple(expected_elements.to_vec()).name();
        let diagnostic =
            Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

        ctx.push(diagnostic_with_syntax_span(diagnostic, fallback_span));
        return InferredExpression::Unknown;
    }

    InferredExpression::Known(Type::Tuple(expected_elements.to_vec()))
}

/// Whether an expected member type stands at surge's degradation sentinel, so a
/// comparison against it proves nothing. Deliberately shallow: this runs for
/// every property of every checked object literal, and the deep walk resolves
/// lazy references — too expensive here, and re-entrant while a literal is
/// mid-check.
fn expected_member_is_degraded(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Union(union) => union.types().iter().any(|member| member.is_unknown()),
        _ => false,
    }
}

/// tsc's excess-property report: the first property the target does not
/// declare, reported once. It runs only after the written properties have
/// checked out — a property that fails against its expected type reports
/// instead — and it takes precedence over the missing-required report.
fn report_excess_property(
    properties: &[ParsedObjectProperty],
    expected_object_type: &surge_ts_types::ObjectType,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    if expected_object_type.allows_string_index_access() || expected_object_type.properties.is_empty()
    {
        return false;
    }
    let Some(property) = properties.iter().find(|property| {
        !property.is_spread && !expected_object_type.contains_property(&property.name)
    }) else {
        return false;
    };

    let diagnostic = Diagnostic::ts2353(
        &property.name,
        &Type::Object(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
            expected_object_type.clone()
        }))
        .name(),
        ctx.file_name.clone(),
    );
    ctx.push(diagnostic_with_syntax_span(
        diagnostic,
        choose_span(
            property.name_span,
            choose_span(property.span, fallback_span),
        ),
    ));
    true
}

fn evaluate_object_literal_with_expected_type(
    properties: &[ParsedObjectProperty],
    expected_object_type: &surge_ts_types::ObjectType,
    fallback_span: Option<SyntaxTextSpan>,
    target_span: Option<SyntaxTextSpan>,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_start = Instant::now();
    let mut inferred_property_types = BTreeMap::new();
    // The empty object type `{}` (no properties, no string index) accepts any
    // object literal without excess-property errors, matching tsc. Library
    // signatures like `Object.keys(o: {})` rely on this.
    // A `{ ...source }` spread contributes `source`'s properties under an empty
    // name; it is neither an excess key nor a single checkable property here, and
    // it may supply required properties we don't track by name. Skip spreads in
    // the excess-property scan and, when any spread is present, in the missing
    // required-property scan below (conservative: under-check rather than emit a
    // false `TS2353`/`TS2741`).
    let has_spread = properties.iter().any(|property| property.is_spread);

    // Set when a property the literal *writes* is compared against an expected
    // member surge could not model. tsc reports one error per literal and stops:
    // a written property that fails is reported at the property, and the
    // missing-required-property report never happens. When the member is a
    // degradation sentinel the comparison passes permissively, so a missing
    // property below would be reported *instead* of the property error tsc
    // reports — the `@ts-expect-error` a test wrote over the property then does
    // not cover it. Withhold the missing-property report in that case.
    let mut degraded_property_comparison = false;

    for property in properties {
        if property.is_spread {
            continue;
        }
        record_object_literal_property_check();
        let expected_property = if let Some(expected_property) =
            expected_object_type.get_property(&property.name).cloned()
        {
            expected_property
        } else if let Some(index_type) = expected_object_type.string_index_type.as_deref().cloned()
        {
            ObjectProperty::required(index_type)
        } else {
            // The target has no such property, so there is no contextual type
            // to check the value against — but the value is still an expression
            // with errors of its own, and tsc reports them alongside the excess
            // report below. Method and accessor shorthand is checked by the
            // inference pass, as in the plain object-literal path.
            if !property.is_method && !property.is_accessor {
                if property.is_shorthand {
                    ctx.shorthand_property_depth += 1;
                }
                let _ = evaluate_expression(
                    &property.value,
                    property.value_span.or(property.span),
                    symbols,
                    ctx,
                );
                if property.is_shorthand {
                    ctx.shorthand_property_depth -= 1;
                }
            }
            continue;
        };

        let expected_property_type = if expected_property.is_optional() {
            surge_ts_types::union_type(vec![
                with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                    expected_property.ty.clone()
                }),
                Type::Undefined,
            ])
        } else {
            with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                expected_property.ty.clone()
            })
        };
        if expected_member_is_degraded(&expected_property.ty) {
            degraded_property_comparison = true;
        }

        // Without `exactOptionalPropertyTypes` an optional property's type
        // *includes* `undefined` — tsc folds it in at declaration resolution —
        // so the contextual type has to carry it. A conditional value is checked
        // branch by branch against the contextual type, and `flag ? x : undefined`
        // fails on its `undefined` branch otherwise.
        let contextual_property_type = expected_property_type.clone();

        // A `get`/`set` accessor is written as a function but *is* the property:
        // contextually type it as one returning the expected type, then compare
        // the accessor's value type (getter return / setter parameter) rather
        // than the accessor function itself.
        let accessor_contextual_type = property.is_accessor.then(|| {
            Type::Function(surge_ts_types::FunctionType::new(
                Vec::new(),
                with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                    contextual_property_type.clone()
                }),
                false,
                0,
            ))
        });
        if property.is_shorthand {
            ctx.shorthand_property_depth += 1;
        }
        let inferred_property = evaluate_expression_with_expected_type(
            &property.value,
            property.value_span.or(property.span),
            Some(
                accessor_contextual_type
                    .as_ref()
                    .unwrap_or(&contextual_property_type),
            ),
            ExpectedTypeDiagnostic::TypeNotAssignable,
            symbols,
            ctx,
        );
        if property.is_shorthand {
            ctx.shorthand_property_depth -= 1;
        }
        let inferred_property = match inferred_property {
            InferredExpression::Known(Type::Function(function_type)) if property.is_accessor => {
                InferredExpression::Known(match function_type.parameters().first() {
                    Some(parameter) => parameter.clone(),
                    None => function_type.return_type().clone(),
                })
            }
            other => other,
        };

        match inferred_property {
            InferredExpression::Known(actual_type) => {
                if actual_type.is_unknown()
                    || crate::checks::call::is_open_instantiation(&actual_type)
                {
                    inferred_property_types.insert(property.name.clone(), Type::Unknown);
                    continue;
                }

                inferred_property_types.insert(
                    property.name.clone(),
                    with_type_copy_reason(TypeCopyReason::ExpectedType, || actual_type.clone()),
                );
                record_assignability_check();
                if !is_assignable_to(&actual_type, &expected_property_type) {
                    // The comparison target carries the optionality-implied
                    // `undefined`, but tsc's elaboration names the property's
                    // written type — a value that is not `undefined` failed
                    // against that, not against the widened union.
                    let reported_target = if actual_type == Type::Undefined {
                        &expected_property_type
                    } else {
                        &expected_property.ty
                    };
                    let actual_type_name = source_display_name(&actual_type, reported_target);
                    let expected_type_name = reported_target.name();
                    let (actual_type_name, expected_type_name) =
                        crate::checks::expr::disambiguated_pair(
                            &actual_type,
                            actual_type_name,
                            reported_target,
                            expected_type_name,
                            &ctx.file_name,
                        );
                    let diagnostic = Diagnostic::ts2322(
                        &actual_type_name,
                        &expected_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        choose_span(
                            property.name_span,
                            choose_span(
                                property.value_span,
                                choose_span(property.span, fallback_span),
                            ),
                        ),
                    ));
                    return InferredExpression::Unknown;
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                inferred_property_types.insert(property.name.clone(), Type::Unknown);
                // The value's own error is already reported and the literal
                // cannot be compared further, but an excess property is a
                // separate report tsc still makes.
                report_excess_property(properties, expected_object_type, fallback_span, ctx);
                return InferredExpression::Unknown;
            }
        }
    }

    if report_excess_property(properties, expected_object_type, fallback_span, ctx) {
        return InferredExpression::Unknown;
    }

    let missing_property_names: Vec<String> = if has_spread || degraded_property_comparison {
        Vec::new()
    } else {
        expected_object_type
            .required_properties()
            .filter(|(property_name, _)| {
                !properties
                    .iter()
                    .any(|property| property.name == property_name.as_ref())
            })
            .map(|(property_name, _)| property_name.to_string())
            .collect()
    };

    if let Some(property_name) = missing_property_names.first() {
        let source_type_name = crate::checks::expr::widen_type(&object_literal_source_type_name(
            properties,
            &inferred_property_types,
        ))
        .name();
        let target_type_name =
            Type::Object(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                expected_object_type.clone()
            }))
            .name();

        // tsc surfaces a missing required property differently for an
        // intersection target: it reports the outer assignability code (the
        // missing property becomes nested elaboration) rather than the
        // standalone TS2741. Mirror that so the reported code matches.
        let diagnostic = if expected_object_type.is_intersection {
            match expected_diagnostic {
                ExpectedTypeDiagnostic::TypeNotAssignable => {
                    Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                ExpectedTypeDiagnostic::ArgumentNotAssignable => {
                    Diagnostic::ts2345(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                ExpectedTypeDiagnostic::SatisfiesNotAssignable => {
                    Diagnostic::ts1360(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
            }
        } else {
            match expected_diagnostic {
                ExpectedTypeDiagnostic::SatisfiesNotAssignable => {
                    Diagnostic::ts1360(&source_type_name, &target_type_name, ctx.file_name.clone())
                }
                _ => missing_properties_diagnostic(
                    property_name,
                    &missing_property_names,
                    &source_type_name,
                    &target_type_name,
                    ctx,
                ),
            }
        };

        ctx.push(diagnostic_with_syntax_span(
            diagnostic,
            choose_span(target_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    }

    let result = InferredExpression::Known(Type::Object(with_type_copy_reason(
        TypeCopyReason::ExpectedType,
        || expected_object_type.clone(),
    )));
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.object_literal_checking += object_start.elapsed()
    });
    result
}

/// tsc names *every* missing required property, and picks the code by how many
/// there are: one is TS2741, two to five are listed in full as TS2739, and six
/// or more list the first four as TS2740 with the rest counted.
fn missing_properties_diagnostic(
    first_missing: &str,
    missing: &[String],
    source_type_name: &str,
    target_type_name: &str,
    ctx: &CheckerContext,
) -> Diagnostic {
    const LISTED_WHEN_TRUNCATED: usize = 4;
    const MAX_LISTED: usize = 5;

    match missing.len() {
        0 | 1 => Diagnostic::ts2741(
            first_missing,
            source_type_name,
            target_type_name,
            ctx.file_name.clone(),
        ),
        count if count <= MAX_LISTED => Diagnostic::ts2739(
            source_type_name,
            target_type_name,
            missing.join(", "),
            ctx.file_name.clone(),
        ),
        count => Diagnostic::ts2740(
            source_type_name,
            target_type_name,
            missing[..LISTED_WHEN_TRUNCATED].join(", "),
            count - LISTED_WHEN_TRUNCATED,
            ctx.file_name.clone(),
        ),
    }
}

fn object_literal_source_type_name(
    properties: &[ParsedObjectProperty],
    inferred_property_types: &BTreeMap<String, Type>,
) -> Type {
    let properties = properties
        .iter()
        .map(|property| {
            let ty = inferred_property_types
                .get(&property.name)
                .cloned()
                .unwrap_or(Type::Unknown);
            (property.name.as_str().into(), ObjectProperty::required(ty))
        })
        .collect::<surge_ts_types::PropertyMap>();

    Type::Object(alloc_object_type(properties, None))
}

fn evaluate_conditional_expression_with_expected_type(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    expected_type: &Type,
    expected_diagnostic: ExpectedTypeDiagnostic,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let ParsedExpression::Conditional {
        condition,
        condition_span,
        when_true,
        when_true_span,
        when_false,
        when_false_span,
    } = expression
    else {
        return evaluate_expression(expression, fallback_span, symbols, ctx);
    };

    // Narrow a discriminated union for each branch (`x.kind === "a" ? … : …`).
    let true_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, true);
    let true_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        true_symbols.as_ref().unwrap_or(symbols),
        true,
        ctx,
    )
    .or(true_symbols);
    let false_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, false);
    let false_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        false_symbols.as_ref().unwrap_or(symbols),
        false,
        ctx,
    )
    .or(false_symbols);
    // A guard on an element access (`typeof xs[0] === 'string' ? xs[0] : …`)
    // narrows the access itself, which no binding's type can carry.
    let true_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        true,
        true_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(true_symbols);
    let false_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        false,
        false_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(false_symbols);
    let true_symbols = true_symbols.as_ref().unwrap_or(symbols);
    let false_symbols = false_symbols.as_ref().unwrap_or(symbols);

    if *expected_type == Type::Any {
        let _ = evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
        let _ = evaluate_expression(
            when_true,
            when_true_span.or(fallback_span),
            true_symbols,
            ctx,
        );
        let _ = evaluate_expression(
            when_false,
            when_false_span.or(fallback_span),
            false_symbols,
            ctx,
        );

        return InferredExpression::Known(Type::Any);
    }

    let condition_result =
        evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
    let true_result = evaluate_expression_with_expected_type(
        when_true,
        when_true_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        true_symbols,
        ctx,
    );
    let false_result = evaluate_expression_with_expected_type(
        when_false,
        when_false_span.or(fallback_span),
        Some(expected_type),
        expected_diagnostic,
        false_symbols,
        ctx,
    );

    let true_branch_span = when_true_span.or(fallback_span);
    let false_branch_span = when_false_span.or(fallback_span);
    let mut has_contextual_mismatch = false;
    let true_branch_type = known_branch_type(&true_result);
    let false_branch_type = known_branch_type(&false_result);
    let branch_types_differ = match (true_branch_type, false_branch_type) {
        (Some(true_type), Some(false_type)) => {
            match (true_type.base_primitive(), false_type.base_primitive()) {
                (Some(true_base), Some(false_base)) => true_base != false_base,
                _ => true_type != false_type,
            }
        }
        _ => false,
    };

    has_contextual_mismatch |= check_conditional_branch_expected_type(
        true_result,
        true_branch_span,
        expected_type,
        expected_diagnostic,
        ctx,
    );
    if !branch_types_differ || !has_contextual_mismatch {
        has_contextual_mismatch |= check_conditional_branch_expected_type(
            false_result,
            false_branch_span,
            expected_type,
            expected_diagnostic,
            ctx,
        );
    }

    if matches!(condition_result, InferredExpression::Unknown) {
        return InferredExpression::Unknown;
    }

    if has_contextual_mismatch {
        return InferredExpression::Unknown;
    }

    InferredExpression::Known(with_type_copy_reason(TypeCopyReason::ExpectedType, || {
        expected_type.clone()
    }))
}

fn check_conditional_branch_expected_type(
    branch_result: InferredExpression,
    branch_span: Option<SyntaxTextSpan>,
    expected_type: &Type,
    expected_diagnostic: ExpectedTypeDiagnostic,
    ctx: &mut CheckerContext,
) -> bool {
    match branch_result {
        InferredExpression::Known(branch_type) => {
            if branch_type.is_unknown() {
                return false;
            }

            if is_assignable_to(&branch_type, expected_type) {
                return false;
            }

            push_expected_type_mismatch(
                &branch_type,
                expected_type,
                branch_span,
                expected_diagnostic,
                ctx,
            );
            true
        }
        InferredExpression::UnresolvedIdentifier { .. } => false,
        InferredExpression::MissingProperty { .. } => false,
        InferredExpression::Unknown => false,
    }
}

fn known_branch_type(branch_result: &InferredExpression) -> Option<&Type> {
    match branch_result {
        InferredExpression::Known(ty) if !ty.is_unknown() => Some(ty),
        _ => None,
    }
}

fn push_expected_type_mismatch(
    source_type: &Type,
    expected_type: &Type,
    span: Option<SyntaxTextSpan>,
    diagnostic_kind: ExpectedTypeDiagnostic,
    ctx: &mut CheckerContext,
) {
    // An instantiation over an open argument (`Mock<T>` with `T` bare) is not
    // settled enough to reject; the argument path already declines it.
    if crate::checks::call::is_open_instantiation(source_type) {
        return;
    }
    let (source_type_name, expected_type_name) = crate::checks::expr::disambiguated_pair(
        source_type,
        source_display_name(source_type, expected_type),
        expected_type,
        expected_type.name(),
        &ctx.file_name,
    );
    let diagnostic = match diagnostic_kind {
        ExpectedTypeDiagnostic::TypeNotAssignable => Diagnostic::ts2322(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
        ExpectedTypeDiagnostic::ArgumentNotAssignable => Diagnostic::ts2345(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
        ExpectedTypeDiagnostic::SatisfiesNotAssignable => Diagnostic::ts1360(
            &source_type_name,
            &expected_type_name,
            ctx.file_name.clone(),
        ),
    };

    ctx.push(diagnostic_with_syntax_span(diagnostic, span));
}
