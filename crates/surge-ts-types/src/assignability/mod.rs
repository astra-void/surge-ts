use std::sync::Arc;

use crate::{FunctionType, ObjectType, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectAssignabilityFailure {
    MissingProperty {
        property_name: String,
    },
    PropertyTypeMismatch {
        property_name: String,
        source_type: Type,
        target_type: Type,
    },
    /// A `private` member related to anything but itself, or a `protected`
    /// one to or from a public member (`propertyRelatedTo`).
    AccessibilityMismatch {
        property_name: String,
    },
}

thread_local! {
    /// Recursion depth of the current `is_assignable_to` evaluation. Lazy nominal
    /// `Type::Reference`s can form cyclic structural graphs (interface A whose member
    /// resolves to B whose member resolves back to A); structural comparison would
    /// otherwise recurse forever following them. The bound breaks such a cycle by
    /// treating the over-deep comparison as assignable — the coinductive choice tsc
    /// makes with its relation-in-progress set.
    static ASSIGNABILITY_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };

    /// Object pairs whose assignability is being decided on the current stack,
    /// keyed by their shared property-map `Arc` pointers (stable across the
    /// memoized `resolve()` of a reference). Mutually-recursive library object
    /// graphs (e.g. DOM `Request`/`RequestInit`, whose members cycle back) make
    /// `object_assignability_failure` re-ask the *same* pair while it is still in
    /// progress; without this the comparison re-descends the cycle from every
    /// sibling property, which is exponential. Re-asking an in-progress pair
    /// answers `true` coinductively — the same answer the depth bound gives, but
    /// at the cycle edge instead of after 200 redundant levels. Cleared when the
    /// outermost `is_assignable_to` returns so a freed `Arc` pointer can never be
    /// reused for a stale entry.
    static OBJECT_ASSIGNABILITY_IN_PROGRESS: std::cell::RefCell<crate::fx::FxHashSet<(usize, usize)>> =
        std::cell::RefCell::new(crate::fx::FxHashSet::default());

    /// Completed-result memo for the current outermost `is_assignable_to` query,
    /// keyed on stable `Arc` identities of both sides. The in-progress set above
    /// only catches cycles; on acyclic DAG-shaped types (the same sub-pair
    /// reached from many sibling properties or union arms) every path re-ran the
    /// full comparison, which is exponential in nesting depth. Cleared with the
    /// in-progress set when the outermost call returns, so a freed `Arc` pointer
    /// can never alias a stale entry.
    static ASSIGNABILITY_RELATION_CACHE: std::cell::RefCell<crate::fx::FxHashMap<(u8, RelationKey, RelationKey), bool>> =
        std::cell::RefCell::new(crate::fx::FxHashMap::default());

    /// Bumped whenever a comparison is answered by assumption (depth-cap or
    /// in-progress coinductive `true`) rather than by inspection. A result whose
    /// subtree consumed an assumption is only valid under that assumption, so it
    /// must not be memoized as definitive — mirroring tsc's `Ternary.Maybe`
    /// handling in its relation cache.
    static ASSIGNABILITY_ASSUMPTION_EVENTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };

    /// Comparisons made under the current outermost `is_assignable_to` query.
    /// The depth bound alone does not bound *work*: a union of generic classes
    /// whose members reference the union again (ast-types' `Type<T>`) fans out
    /// at every level, and every assumption on the way voids the relation memo,
    /// so the comparison is exponential below the cap. Past the budget the
    /// remaining comparisons are answered by assumption, the same coinductive
    /// answer the cap gives.
    static ASSIGNABILITY_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };

    /// Apparent shapes built for one comparison (see
    /// [`array_like_apparent_object`]). Their member types key the relation
    /// memo by payload address, so they are held until the outermost query
    /// ends: a freed member could otherwise hand its address to the next
    /// shape's and be answered from the stale entry.
    static SYNTHESIZED_TARGETS: std::cell::RefCell<Vec<Type>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// The relation the current outermost query is being decided under. Constant
    /// for the duration of one query; [`is_comparable_to`] sets and restores it.
    static CURRENT_RELATION: std::cell::Cell<Relation> =
        const { std::cell::Cell::new(Relation::Assignable) };
}

fn current_relation() -> Relation {
    CURRENT_RELATION.with(std::cell::Cell::get)
}

/// Which relation a comparison is being decided under, mirroring tsc's
/// `assignableRelation` / `comparableRelation`. The engine is a set of free
/// functions sharing thread-local state (depth, in-progress set, memo), so the
/// relation lives alongside them rather than being threaded through every
/// signature — the same role `Relater.relation` plays in `relater.go`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Assignable,
    /// tsc's comparable relation: laxer than assignability, and the relation an
    /// `x as T` conversion is checked under. The difference that matters is a
    /// union *source*, which is related when *some* constituent is rather than
    /// every one (`relater.go`, `someTypeRelatedToType` vs
    /// `eachTypeRelatedToType`).
    Comparable,
}

const MAX_ASSIGNABILITY_DEPTH: u32 = 200;
const MAX_ASSIGNABILITY_STEPS: u64 = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RelationKey {
    tag: u8,
    parts: [usize; 6],
}

/// Stable identity for memoizing a comparison side. Every field that can change
/// the assignability verdict must contribute (properties, both index signatures,
/// call and construct signatures, `alias_id` for the nominal fast path); types without a
/// shared-`Arc` identity return `None` and are simply not memoized.
fn relation_key(ty: &Type) -> Option<RelationKey> {
    match ty {
        Type::Object(object) => Some(RelationKey {
            tag: 1,
            parts: [
                Arc::as_ptr(&object.properties) as usize,
                object
                    .string_index_type
                    .as_ref()
                    .map_or(0, |index| Arc::as_ptr(index) as usize),
                object
                    .number_index_type
                    .as_ref()
                    .map_or(0, |index| Arc::as_ptr(index) as usize),
                object
                    .call_signature()
                    .map_or(0, |signature| signature.payload_address()),
                object
                    .construct_signature()
                    .map_or(0, |signature| signature.payload_address()),
                object
                    .alias_id
                    .as_ref()
                    .map_or(0, |id| id.as_ref().as_ptr() as usize),
            ],
        }),
        Type::Union(union) => Some(RelationKey {
            tag: 2,
            parts: [union.payload_address(), 0, 0, 0, 0, 0],
        }),
        Type::Function(function) => Some(RelationKey {
            tag: 3,
            parts: [function.payload_address(), 0, 0, 0, 0, 0],
        }),
        _ => None,
    }
}

fn record_assignability_assumption() {
    ASSIGNABILITY_ASSUMPTION_EVENTS.with(|events| events.set(events.get() + 1));
}

/// Widest target union a discriminated distribution is attempted against. The
/// cap keeps a pathological union from turning one failed comparison into a
/// quadratic sweep.
const MAX_DISCRIMINATED_UNION_MEMBERS: usize = 32;

/// tsc's `typeRelatedToDiscriminatedType`: an object source is related to a
/// union target when every combination of the values its discriminant
/// properties can hold picks target members, and each picked member accepts
/// the rest of the source. `{ done: boolean; value: T }` relates to
/// `{ done: true; value: T } | { done: false; value: T }` although neither
/// member accepts it whole.
///
/// A discriminant is a property the target's members declare with differing
/// types, at least one of them a unit type (a literal, `true`/`false`,
/// `undefined` or `null`). More than 25 combinations is too complex, as in tsc.
///
/// Runs only after the plain member-wise check already failed.
fn discriminated_union_assignable(from: &Type, to_union: &crate::UnionType) -> bool {
    let Type::Object(from_object) = from else {
        return false;
    };
    let members = to_union.types();
    if members.len() > MAX_DISCRIMINATED_UNION_MEMBERS {
        return false;
    }
    let is_unit = |ty: &Type| {
        matches!(
            ty,
            Type::StringLiteral(_)
                | Type::NumberLiteral(_)
                | Type::BooleanLiteral(_)
                | Type::Boolean
                | Type::Undefined
                | Type::Null
        )
    };
    let has_unit_part = |ty: &Type| match ty {
        Type::Union(union) => union.types().iter().any(is_unit),
        other => is_unit(other),
    };
    let distributed = |ty: &Type| -> Vec<Type> {
        let mut values = Vec::new();
        let mut push = |ty: &Type| match ty {
            Type::Boolean => {
                values.push(Type::BooleanLiteral(true));
                values.push(Type::BooleanLiteral(false));
            }
            other => values.push(other.clone()),
        };
        match ty {
            Type::Union(union) => union.types().iter().for_each(&mut push),
            other => push(other),
        }
        values
    };

    // Answered from the source alone first: peeling every member is the
    // expensive part, and most failed object-to-union comparisons have no
    // candidate discriminant at all.
    if !from_object.properties.values().any(|property| has_unit_part(&property.ty)) {
        return false;
    }
    let peeled_members: Vec<Type> = members.iter().map(Type::peeled).collect();
    let member_property = |member: &Type, name: &str| -> Option<Type> {
        let Type::Object(object) = member else {
            return None;
        };
        let property = object.properties.get(name)?;
        Some(if property.is_optional() {
            crate::union_type(vec![property.ty.clone(), Type::Undefined])
        } else {
            property.ty.clone()
        })
    };

    let mut discriminants: Vec<(&str, Vec<Type>)> = Vec::new();
    let mut combinations = 1usize;
    for (name, property) in from_object.properties.iter() {
        let declared: Vec<Type> = peeled_members
            .iter()
            .filter_map(|member| member_property(member, name))
            .collect();
        let uniform = declared.windows(2).all(|pair| pair[0] == pair[1]);
        if declared.is_empty() || uniform || !declared.iter().any(has_unit_part) {
            continue;
        }
        let values = distributed(&property.ty);
        combinations = combinations.saturating_mul(values.len());
        if combinations > 25 || values.is_empty() {
            return false;
        }
        discriminants.push((name.as_ref(), values));
    }
    if discriminants.is_empty() {
        return false;
    }

    let mut matching: Vec<usize> = Vec::new();
    for index in 0..combinations {
        let mut combination = Vec::with_capacity(discriminants.len());
        let mut n = index;
        for (_, values) in discriminants.iter().rev() {
            combination.push(&values[n % values.len()]);
            n /= values.len();
        }
        combination.reverse();
        let mut has_match = false;
        for (member_index, member) in peeled_members.iter().enumerate() {
            let fits = discriminants.iter().zip(&combination).all(|((name, _), value)| {
                member_property(member, name).is_some_and(|declared| is_assignable_to(value, &declared))
            });
            if fits {
                has_match = true;
                if !matching.contains(&member_index) {
                    matching.push(member_index);
                }
            }
        }
        if !has_match {
            return false;
        }
    }

    let excluded: Vec<&str> = discriminants.iter().map(|(name, _)| *name).collect();
    let without_discriminants = |object: &ObjectType| {
        let mut properties = (*object.properties).clone();
        for name in &excluded {
            properties.shift_remove(*name);
        }
        let mut stripped = ObjectType::new(properties, object.string_index_type.as_deref().cloned())
            .with_number_index_type(object.number_index_type.as_deref().cloned());
        if let Some(signature) = object.call_signature() {
            stripped = stripped.with_call_signature(signature.clone());
        }
        if let Some(signature) = object.construct_signature() {
            stripped = stripped.with_construct_signature(signature.clone());
        }
        Type::Object(stripped)
    };
    let source = without_discriminants(from_object);
    matching.iter().all(|&member_index| match &peeled_members[member_index] {
        Type::Object(member) => is_assignable_to(&source, &without_discriminants(member)),
        _ => false,
    })
}

/// tsc's `isTypeComparableTo`. Same engine as [`is_assignable_to`], decided
/// under the comparable relation: `x as T` is legal when the two types overlap
/// in either direction, and overlap is laxer than assignability.
///
/// Nested calls keep whatever relation the outermost query established, exactly
/// as a `Relater` carries one relation through a whole comparison.
pub fn is_comparable_to(from: &Type, to: &Type) -> bool {
    let previous = CURRENT_RELATION.with(|relation| relation.replace(Relation::Comparable));
    let result = is_assignable_to(from, to);
    CURRENT_RELATION.with(|relation| relation.set(previous));
    result
}

pub fn is_assignable_to(from: &Type, to: &Type) -> bool {
    struct DepthGuard;
    impl Drop for DepthGuard {
        fn drop(&mut self) {
            ASSIGNABILITY_DEPTH.with(|depth| {
                let next = depth.get().saturating_sub(1);
                depth.set(next);
                if next == 0 {
                    OBJECT_ASSIGNABILITY_IN_PROGRESS.with(|set| set.borrow_mut().clear());
                    ASSIGNABILITY_RELATION_CACHE.with(|cache| cache.borrow_mut().clear());
                    ASSIGNABILITY_STEPS.with(|steps| steps.set(0));
                    SYNTHESIZED_TARGETS.with(|targets| targets.borrow_mut().clear());
                }
            });
        }
    }
    let depth = ASSIGNABILITY_DEPTH.with(|depth| {
        let next = depth.get() + 1;
        depth.set(next);
        next
    });
    let _guard = DepthGuard;
    if depth > MAX_ASSIGNABILITY_DEPTH {
        record_assignability_assumption();
        return true;
    }
    let steps = ASSIGNABILITY_STEPS.with(|steps| {
        let next = steps.get() + 1;
        steps.set(next);
        next
    });
    if steps > MAX_ASSIGNABILITY_STEPS {
        record_assignability_assumption();
        return true;
    }

    // `isSimpleTypeRelatedTo` (relater.go:211) rejects a `never` target before
    // the rule that relates an `any` source to everything, so `any` and tsc's
    // error type (an `any`) do not relate to `never`. surge's degradation
    // sentinel is `Type::Unknown`, which stays permissive.
    if matches!(from, Type::Any | Type::ErrorType) && matches!(to, Type::Never) {
        return false;
    }

    // The comparable relation is mostly bidirectional: before any structural
    // step, a pair also relates when the target is simply related to the
    // source (`isRelatedTo`, relater.go:2694).
    if current_relation() == Relation::Comparable
        && !matches!(to, Type::Never)
        && is_simple_type_related_to(to, from)
    {
        return true;
    }

    // Only the outermost relation: a promise nested in a member may have been
    // collapsed to its awaited type on one side alone.
    if depth == 1
        && ((promise_reference(to) && definitely_not_thenable(from))
            || (promise_reference(from) && definitely_not_thenable(to)))
    {
        return false;
    }

    if from != to
        && current_relation_admits_anything(to)
    {
        return true;
    }
    if from != to
        && let Some(related) = type_variable_related(from, to)
    {
        return related;
    }

    if from == to
        || matches!(from, Type::Any)
        || matches!(from, Type::Never)
        || matches!(to, Type::Any)
        || to.is_unknown()
        // Sentinel `Unknown` (NOT the `unknown` keyword, which is
        // `GenuineUnknown`) marks a type surge could not model — e.g.
        // `SubmitEvent.nativeEvent`, whose `NativeSubmitEvent` alias collides
        // with the enclosing declaration and degrades. tsc compares the real
        // type there, so failing on a degraded *source* turns every unmodelled
        // corner into a false-positive cascade. The same leniency already
        // applies to sentinel arguments in the same-generic fast path below.
        // tsc's `errorType` is `any`, so it flows both ways like the sentinel.
        || matches!(from, Type::Unknown | Type::ErrorType | Type::TypeParameter(_))
    {
        return true;
    }
    if matches!(from, Type::Null | Type::Undefined)
        && !crate::strict_null_checks()
        && !matches!(to, Type::Never)
    {
        return true;
    }
    if current_relation() == Relation::Assignable && has_no_common_properties(from, to) {
        return false;
    }

    // An intersection relates when some constituent does
    // (`someTypeRelatedToType`). A merged intersection keeps only its object
    // side's members, so a branded primitive (`"id" & { __brand: "id" }`) is
    // judged through the primitive operand it recorded.
    if let Type::Object(object) = from
        && object.is_intersection
        && object.intersection_operands.as_deref().is_some_and(|operands| {
            operands.iter().any(|operand| {
                !matches!(operand, Type::Reference(_) | Type::Object(_)) && is_assignable_to(operand, to)
            })
        })
    {
        return true;
    }

    // tsc lets any `number` flow into a numeric `enum` — its own
    // `Flags.A | Flags.B` is typed `number`, and the bitwise combination is the
    // normal way to build a flag argument. The reverse (a string into a string
    // enum) is rejected, which is why only the numeric marker opens this.
    // A number *literal* is not covered: it must match a member's value
    // (`isSimpleTypeRelatedTo`), which the structural comparison against the
    // enum's member union below decides — a computed member resolves to
    // `number` and so still accepts any literal. `number | 0` (from `x ?? 0`)
    // is `number` once tsc's subtype reduction drops the literal.
    if matches!(to, Type::Reference(reference) if reference.numeric_enum)
        && match from {
            Type::Number => true,
            Type::Union(union) => {
                union.types().iter().any(|member| matches!(member, Type::Number))
                    && matches!(from.base_primitive(), Some(Type::Number))
            }
            _ => false,
        }
    {
        return true;
    }

    // A `from` reference is resolved and recursed into below; computing its base
    // primitive here would force a full structural clone of the resolved type
    // (`base_primitive` peels references) only to almost always find it is not a
    // primitive. The recursion re-checks the base primitive on the resolved shape,
    // so skipping references here is behaviour-preserving and avoids the clone —
    // this is the dominant per-peel cost on conditional/mapped-type-heavy programs.
    if !matches!(from, Type::Reference(_))
        && from
            .base_primitive()
            .as_ref()
            .is_some_and(|base| base == to)
    {
        return true;
    }

    let cache_key = match (relation_key(from), relation_key(to)) {
        (Some(from_key), Some(to_key)) => {
            // The relation is part of the key: the same pair legitimately gets
            // different answers under the comparable relation, and a key that
            // omitted it would let one relation answer for the other.
            let pair = (current_relation() as u8, from_key, to_key);
            if let Some(result) =
                ASSIGNABILITY_RELATION_CACHE.with(|cache| cache.borrow().get(&pair).copied())
            {
                return result;
            }
            Some(pair)
        }
        _ => None,
    };
    let assumptions_before = ASSIGNABILITY_ASSUMPTION_EVENTS.with(std::cell::Cell::get);

    let result = assignability_arms(from, to);

    if let Some(pair) = cache_key
        && ASSIGNABILITY_ASSUMPTION_EVENTS.with(std::cell::Cell::get) == assumptions_before
    {
        ASSIGNABILITY_RELATION_CACHE.with(|cache| {
            cache.borrow_mut().insert(pair, result);
        });
    }
    result
}

/// An un-collapsed lib `Promise<T>` / `PromiseLike<T>` reference. It peels to
/// its awaited `T`, which would relate it to every plain `T`.
fn promise_reference(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    reference.enum_owner.is_none()
        && !reference.arguments.is_empty()
        && matches!(
            reference.display.split('<').next(),
            Some("Promise" | "PromiseLike")
        )
}

fn definitely_not_thenable(ty: &Type) -> bool {
    match ty {
        Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(definitely_not_thenable),
        _ => false,
    }
}

/// `isSimpleTypeRelatedTo`'s last rule: under `strictNullChecks` anything is
/// assignable (and comparable) to a union holding `undefined`, `null` and the
/// empty object type — the union `unknown` stands for (`isUnknownLikeUnionType`).
fn current_relation_admits_anything(to: &Type) -> bool {
    let Type::Union(union) = to else {
        return false;
    };
    crate::strict_null_checks()
        && union.types().iter().any(|member| matches!(member, Type::Undefined))
        && union.types().iter().any(|member| matches!(member, Type::Null))
        && union.types().iter().any(is_empty_anonymous_object)
}

/// relater.go `isSimpleTypeRelatedTo` for the assignable and comparable
/// relations: the flag-level rules, decided without looking at structure.
/// Enum members and types are nominal references here, so their literal
/// values are read through the reference rather than by peeling it.
fn is_simple_type_related_to(source: &Type, target: &Type) -> bool {
    if matches!(target, Type::Any | Type::ErrorType) || matches!(source, Type::Never) {
        return true;
    }
    if target.is_unmodelled() {
        return true;
    }
    if matches!(target, Type::Never) {
        return false;
    }
    let related_by_kind = match target {
        Type::String => is_string_like(source),
        Type::Number => is_number_like(source),
        Type::BigInt => matches!(source, Type::BigInt),
        Type::Boolean => matches!(source, Type::Boolean | Type::BooleanLiteral(_)),
        Type::Symbol => {
            matches!(source, Type::Symbol)
                || matches!(source, Type::Reference(reference) if reference.is_unique_symbol())
        }
        _ => false,
    };
    if related_by_kind {
        return true;
    }
    if matches!(target, Type::StringLiteral(_) | Type::NumberLiteral(_))
        && enum_member_value(source).is_some_and(|value| value == *target)
    {
        return true;
    }
    if let (Type::Reference(source_ref), Type::Reference(target_ref)) = (source, target)
        && let (Some(source_enum), Some(target_enum)) = (&source_ref.enum_owner, &target_ref.enum_owner)
        && source_enum == target_enum
        && *source_ref.resolve_arc() == *target_ref.resolve_arc()
    {
        return true;
    }
    let strict = crate::strict_null_checks();
    if matches!(source, Type::Undefined)
        && (!strict && !is_union_or_intersection(target) || matches!(target, Type::Undefined | Type::Void))
    {
        return true;
    }
    if matches!(source, Type::Null) && (!strict && !is_union_or_intersection(target) || matches!(target, Type::Null)) {
        return true;
    }
    if matches!(target, Type::Object(object) if object.non_primitive && !object.is_intersection)
        && is_object_type(source)
    {
        return true;
    }
    if matches!(source, Type::Any | Type::ErrorType) {
        return true;
    }
    let numeric_enum = |ty: &Type| matches!(ty, Type::Reference(reference) if reference.numeric_enum);
    if matches!(source, Type::Number)
        && numeric_enum(target)
        && !matches!(&*resolved_reference(target), Type::Union(_))
    {
        return true;
    }
    if let Type::NumberLiteral(_) = source
        && numeric_enum(target)
        && match &*resolved_reference(target) {
            Type::Number => true,
            literal @ Type::NumberLiteral(_) => literal == source,
            _ => false,
        }
    {
        return true;
    }
    current_relation_admits_anything(target)
}

fn resolved_reference(ty: &Type) -> Arc<Type> {
    match ty {
        Type::Reference(reference) => reference.resolve_arc(),
        other => Arc::new(other.clone()),
    }
}

/// tsc's `StringLike` flags: `string`, a string literal (a string enum
/// member's included), a template literal or a string mapping.
fn is_string_like(ty: &Type) -> bool {
    match ty {
        Type::String | Type::StringLiteral(_) => true,
        Type::Reference(reference) => {
            crate::is_template_literal_type(ty)
                || crate::string_mapping_parts(ty).is_some()
                || reference.enum_owner.is_some()
                    && matches!(&*reference.resolve_arc(), Type::StringLiteral(_))
        }
        _ => false,
    }
}

/// tsc's `NumberLike` flags: `number`, a number literal, a numeric enum
/// member, or an enum whose members are computed (`TypeFlagsEnum`). An enum of
/// literal members is their union, which is not number-like by its flags.
fn is_number_like(ty: &Type) -> bool {
    match ty {
        Type::Number | Type::NumberLiteral(_) => true,
        Type::Reference(reference) => {
            reference.numeric_enum && !matches!(&*reference.resolve_arc(), Type::Union(_))
        }
        _ => false,
    }
}

/// The literal an enum member reference stands for.
fn enum_member_value(ty: &Type) -> Option<Type> {
    let Type::Reference(reference) = ty else {
        return None;
    };
    reference.enum_owner.as_ref()?;
    match &*reference.resolve_arc() {
        literal @ (Type::StringLiteral(_) | Type::NumberLiteral(_)) => Some(literal.clone()),
        _ => None,
    }
}

fn is_union_or_intersection(ty: &Type) -> bool {
    match ty {
        Type::Union(_) => true,
        Type::Object(object) => object.is_intersection,
        Type::Reference(reference) => {
            matches!(&*reference.resolve_arc(), Type::Union(_) | Type::Object(ObjectType { is_intersection: true, .. }))
        }
        _ => false,
    }
}

/// tsc's `TypeFlagsObject`: an object, function, array or tuple type — not
/// the `object` keyword, an intersection, or a primitive a reference names.
fn is_object_type(ty: &Type) -> bool {
    match ty {
        Type::Object(object) => !object.non_primitive && !object.is_intersection,
        Type::Function(_) | Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) => true,
        Type::Reference(reference) => {
            reference.is_readonly_array()
                || reference.enum_owner.is_none()
                    && !reference.is_unique_symbol()
                    && !crate::is_template_literal_type(ty)
                    && crate::string_mapping_parts(ty).is_none()
                    && is_object_type(&reference.resolve_arc())
        }
        _ => false,
    }
}

/// tsc's `isWeakType` for a plain object: members, every one optional, and no
/// signature or index signature.
fn is_weak_object(object: &ObjectType) -> bool {
    !object.properties.is_empty()
        && object.properties.values().all(crate::ObjectProperty::is_optional)
        && object.string_index_type.is_none()
        && object.number_index_type.is_none()
        && object.call_signature().is_none()
        && object.construct_signature().is_none()
        && !object.is_intersection
        && !object.synthetic_open_index
        && !object.non_primitive
}

/// tsc's `isUnitType` for a source with an apparent type to list members of:
/// a literal, an enum member or a unique symbol (`undefined` and `null` have
/// no members). `boolean` is tsc's `false | true`, two unit types whose
/// apparent type is the same, so it answers as either member does.
fn is_unit_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) | Type::Boolean => true,
        Type::Reference(reference) => reference.is_unique_symbol() || enum_member_value(ty).is_some(),
        _ => false,
    }
}

fn is_empty_anonymous_object(ty: &Type) -> bool {
    matches!(ty, Type::Object(object)
        if object.properties.is_empty()
            && object.string_index_type.is_none()
            && object.number_index_type.is_none()
            && object.call_signature().is_none()
            && object.construct_signature().is_none()
            && !object.non_primitive
            && !object.is_intersection
            && object.alias_id.is_none())
}

/// The operands of an intersection holding a type variable of the body being
/// checked: `T & X` narrowed (memberless, see
/// `type_variable::intersect_type_variable`) or resolved (the merged members
/// of the other operands, with the variable recorded beside them).
fn type_variable_intersection_operands(ty: &Type) -> Option<&[Type]> {
    let Type::Object(object) = ty else {
        return None;
    };
    let operands = object.intersection_operands.as_deref()?;
    (object.is_intersection && operands.iter().any(Type::is_type_variable)).then_some(operands)
}

/// A memberless `T & X`: its operands are the whole type.
fn is_narrowed_type_variable(ty: &Type) -> bool {
    matches!(ty, Type::Object(object) if object.properties.is_empty() && object.string_index_type.is_none())
}

/// The variable operands' constraints cut down by the other operands: a union
/// constraint loses the members an operand rules out — `{}` rules out the
/// nullish ones (`NonNullable<T>` over `T extends string | undefined` is
/// `string`), a primitive operand every member not of it. `None` when no
/// variable operand has a constraint to combine.
fn effective_intersection_constraint(operands: &[Type]) -> Option<Type> {
    let constraints: Vec<Type> = operands
        .iter()
        .filter_map(|operand| match operand {
            Type::TypeParameter(parameter) => crate::type_variable::active_constraint(parameter).flatten(),
            _ => None,
        })
        .collect();
    let mut members: Vec<Type> = constraints
        .iter()
        .flat_map(|constraint| match constraint {
            Type::Union(union) => union.types().to_vec(),
            other => vec![other.clone()],
        })
        .collect();
    if members.is_empty() {
        return None;
    }
    for operand in operands.iter().filter(|operand| !matches!(operand, Type::TypeParameter(_))) {
        if is_empty_anonymous_object(operand) {
            members.retain(|member| !matches!(member, Type::Null | Type::Undefined | Type::Void));
        } else if operand.base_primitive().is_some() && !matches!(operand, Type::Object(_)) {
            members.retain(|member| is_assignable_to(member, operand));
        }
    }
    Some(if members.is_empty() { Type::Never } else { crate::union_type(members) })
}

fn some_type(ty: &Type, predicate: impl Fn(&Type) -> bool) -> bool {
    match ty {
        Type::Union(union) => union.types().iter().any(predicate),
        other => predicate(other),
    }
}

fn active_variable(ty: &Type) -> Option<(&crate::TypeParameterType, Option<Type>)> {
    match ty {
        Type::TypeParameter(parameter) => {
            crate::type_variable::active_constraint(parameter).map(|constraint| (parameter, constraint))
        }
        _ => None,
    }
}

/// tsc's relation for a type variable of the body being checked
/// (`structuredTypeRelatedToWorker`, relater.go): a variable *source* relates
/// through its constraint (`unknown` when it has none) once no target union
/// member is the variable itself; a variable *target* admits nothing but
/// itself, `never`, `any`, and a variable whose constraint leads to it — `T`
/// "could be instantiated with an arbitrary type". `None` when neither side is
/// such a variable. Placeholders and the degradation sentinel stay permissive
/// on either side: they are surge's gaps, not types.
fn type_variable_related(from: &Type, to: &Type) -> Option<bool> {
    // An intersection target needs every constituent (`typeRelatedToEachType`);
    // a source relates when some constituent does (`someTypeRelatedToType`), or
    // failing that through the constraint the constituents combine to
    // (`getEffectiveConstraintOfIntersection`).
    // A merged intersection also answers structurally through its members, so
    // only a failure on a memberless one is final.
    if let Some(operands) = type_variable_intersection_operands(to) {
        let variables_related = operands
            .iter()
            .filter(|operand| operand.is_type_variable() || is_narrowed_type_variable(to))
            .all(|operand| is_assignable_to(from, operand));
        if !variables_related || is_narrowed_type_variable(to) {
            return Some(variables_related);
        }
    }
    if let Some(operands) = type_variable_intersection_operands(from) {
        if operands.iter().any(|operand| is_assignable_to(operand, to))
            || effective_intersection_constraint(operands).is_some_and(|constraint| is_assignable_to(&constraint, to))
        {
            return Some(true);
        }
        if is_narrowed_type_variable(from) {
            return Some(false);
        }
    }
    let source = active_variable(from);
    let target_is_variable = active_variable(to).is_some();
    if source.is_none() && !target_is_variable {
        return None;
    }
    if matches!(from, Type::Any | Type::Never | Type::Unknown | Type::ErrorType)
        || matches!(to, Type::Any | Type::Unknown | Type::ErrorType | Type::GenuineUnknown)
        || matches!(from, Type::TypeParameter(_)) && source.is_none()
        || matches!(to, Type::TypeParameter(_)) && !target_is_variable
    {
        return Some(true);
    }
    if matches!(from, Type::Null | Type::Undefined) && !crate::strict_null_checks() {
        return Some(true);
    }
    if let Some((_, constraint)) = source {
        if let Type::Union(to_union) = to
            && to_union.types().iter().any(|member| member == from)
        {
            return Some(true);
        }
        // relater.go `structuredTypeRelatedToWorker`: comparability is mostly
        // bidirectional, so a type parameter is comparable to another only
        // through a constraint that itself holds a type parameter.
        if current_relation() == Relation::Comparable && target_is_variable {
            return Some(constraint.is_some_and(|constraint| {
                constraint != *from && some_type(&constraint, |member| matches!(member, Type::TypeParameter(_)))
                    && is_assignable_to(&constraint, to)
            }));
        }
        // `T extends T` is a circular constraint tsc reports and drops.
        let constraint = constraint.filter(|constraint| constraint != from).unwrap_or(Type::GenuineUnknown);
        return Some(is_assignable_to(&constraint, to));
    }
    Some(match from {
        Type::Union(from_union) => union_source_related(from_union, to),
        Type::Reference(reference) => is_assignable_to(&reference.resolve_arc(), to),
        _ => false,
    })
}

fn assignability_arms(from: &Type, to: &Type) -> bool {
    // relater.go `isRelatedTo`: the comparable relation skips the weak type
    // check (`isPerformingCommonPropertyChecks`) except for a unit source,
    // which must still share a property with an all-optional target.
    if current_relation() == Relation::Comparable
        && let Type::Object(target) = to
        && is_weak_object(target)
        && is_unit_literal(from)
        && !target.properties.keys().any(|name| from.get_property_access_type(name).is_some())
    {
        return false;
    }

    // A string mapping target (`Uppercase<string>`): the same mapping relates
    // by what it maps, anything else has to be a member of it
    // (`isMemberOfStringMapping`).
    if let Some((target_kind, target_inner)) = crate::string_mapping_parts(to) {
        let source = crate::peel_to_pattern_literal(from);
        if let Some((source_kind, source_inner)) = crate::string_mapping_parts(&source) {
            return source_kind == target_kind && is_assignable_to(source_inner, target_inner);
        }
        return match &source {
            Type::Union(union) => union_source_related(union, to),
            Type::Any | Type::Never | Type::Unknown | Type::ErrorType | Type::TypeParameter(_) => true,
            other => crate::is_member_of_string_mapping(other, to),
        };
    }

    // A template literal target relates by pattern (tsc's
    // `isTypeMatchedByTemplateLiteralType`), before either side is peeled: the
    // pattern resolves to `string`, which would accept every string literal and
    // reject nothing.
    if let Some((texts, types)) = crate::template_literal_parts(to) {
        let source = crate::peel_to_pattern_literal(from);
        return match &source {
            Type::Union(union) => union_source_related(union, to),
            Type::Any | Type::Never | Type::Unknown | Type::ErrorType | Type::TypeParameter(_) => true,
            other => match crate::template_literal_parts(other) {
                // relater.go: two template literal types are comparable
                // unless their fixed texts already rule it out.
                Some((source_texts, _)) if current_relation() == Relation::Comparable => {
                    !template_literal_types_definitely_unrelated(&source_texts, &texts)
                }
                _ => crate::is_type_matched_by_template_literal(other, &texts, &types),
            },
        };
    }

    // Enum types are nominal (`isEnumTypeRelatedTo`): a member of one enum
    // never relates to another enum, even where the values coincide.
    if let (Type::Reference(from_ref), Type::Reference(to_ref)) = (from, to)
        && let (Some(from_enum), Some(to_enum)) = (&from_ref.enum_owner, &to_ref.enum_owner)
        && from_enum != to_enum
    {
        return false;
    }
    // Relate a union source to an enum member by member, before the target
    // is peeled to its values and the members' enum identity is lost.
    if let (Type::Union(from_union), Type::Reference(to_ref)) = (from, to)
        && to_ref.enum_owner.is_some()
    {
        return union_source_related(from_union, to);
    }
    // Nominal identity: two objects resolved from the same non-generic named
    // declaration are the same type, even if one expanded to a structurally
    // different shape (a deeply cyclic library type can resolve to different
    // depths at different sites). This mirrors tsc's named-type handling.
    if let (Type::Object(from_obj), Type::Object(to_obj)) = (from, to) {
        if let (Some(from_id), Some(to_id)) = (&from_obj.alias_id, &to_obj.alias_id) {
            if from_id == to_id {
                return true;
            }
        }
    }

    // Two instantiations of the *same* generic declaration compare by their type
    // arguments rather than by their (often deeply self-referential) structural
    // expansion. tsc treats `Foo<A>` assignable to `Foo<B>` when the arguments are
    // pairwise compatible — an `any` argument matches anything in either
    // direction. A `unknown` *source* argument is also accepted: it is surge's
    // sentinel for a generic the checker could not infer (a method's own type
    // parameter, `pipeThrough<T>(...): ReadableStream<T>`), so failing it
    // structurally would be a false positive. Structural comparison of two
    // expansions that differ only in an `any`/`unknown` argument is exactly where
    // self-referential library generics (`Uint8Array`, `ReadableStream`, `Set`)
    // spuriously diverge.
    if let (Type::Reference(from_ref), Type::Reference(to_ref)) = (from, to) {
        if from_ref.id == to_ref.id && from_ref.arguments.len() == to_ref.arguments.len() {
            let arguments_compatible =
                from_ref
                    .arguments
                    .iter()
                    .zip(to_ref.arguments.iter())
                    .all(|(from_arg, to_arg)| {
                        // A type variable of the body being checked is a type,
                        // not a gap: `Foo<U>` is no `Foo<T>` unless `U` is a `T`.
                        matches!(from_arg, Type::Any | Type::Unknown | Type::ErrorType | Type::GenuineUnknown)
                            || (matches!(from_arg, Type::TypeParameter(_)) && !from_arg.is_type_variable())
                            || matches!(to_arg, Type::Any)
                            || is_assignable_to(from_arg, to_arg)
                    });
            if arguments_compatible {
                return true;
            }
            // tsc relates two instantiations of one alias by the measured
            // variance of its parameters, and `Record<K, T>`'s `K` — a mapped
            // type's key — measures contravariant: `Record<string, X>` is
            // assignable to `Record<"a", X>` although no `a` is declared.
            if from_ref.id.split('\u{0}').next_back() == Some("Record")
                && let ([from_key, from_value], [to_key, to_value]) =
                    (&*from_ref.arguments, &*to_ref.arguments)
                && is_assignable_to(to_key, from_key)
                && (matches!(from_value, Type::Any) || is_assignable_to(from_value, to_value))
            {
                return true;
            }
        }
    }

    // A reference assignable to a union member must be tried *before* the source
    // reference is resolved to its structural form below: otherwise `Set<any>`
    // against `Set<string> | undefined` would resolve `Set<any>` structurally and
    // compare it to the `Set<string>` member structurally, losing the nominal
    // `any`-argument shortcut above. Scoped to a `from` reference so a `from` union
    // still flows through the all-members `(Union, _)` arm.
    if let (Type::Reference(_), Type::Union(to_union)) = (from, to) {
        if to_union
            .types()
            .iter()
            .any(|to_ty| is_assignable_to(from, to_ty))
        {
            return true;
        }
    }

    // A unique symbol admits only itself: another unique symbol, or the `symbol`
    // either one widens to, is not it. A union source is left to the
    // every-member arm below.
    if let Type::Reference(target) = to
        && target.is_unique_symbol()
        && !matches!(from, Type::Union(_))
    {
        return matches!(from, Type::Reference(source) if source.id == target.id);
    }

    // Nominal references compare nominally first (same declaration + arguments is
    // handled by the `from == to` fast path above); anything else falls back to
    // comparing the structural expansion, so a reference stays interchangeable
    // with its expanded shape without forcing eager expansion at construction.
    if let Type::Reference(reference) = from {
        // A written tuple relates to another tuple by the element flags it
        // records, which its peeled shape no longer shows.
        if let Some((elements, min_length)) = reference.written_tuple()
            && let Some(related) = tuple_source_related(&fixed_tuple_kinds(elements, min_length), to)
        {
            return related;
        }
        // A pattern source against a target that only *names* a pattern (an
        // annotation's lazy `Capitalize<string>`): resolve the target first,
        // or the source peels to `string` below and the pattern is gone.
        if matches!(to, Type::Reference(_))
            && (crate::is_template_literal_type(from) || crate::string_mapping_parts(from).is_some())
        {
            let target = crate::peel_to_pattern_literal(to);
            if crate::is_template_literal_type(&target)
                || crate::string_mapping_parts(&target).is_some()
            {
                return is_assignable_to(from, &target);
            }
        }
        // A readonly array or tuple is not assignable to a mutable one: the
        // mutable surface has `push`/`splice` the readonly one lacks. A union
        // target's members were each tried against the readonly source above
        // (`typeRelatedToSomeType`); retrying them with the mutable shape it
        // resolves to would accept `readonly T[]` as `T[] | undefined`. A
        // target reference is read first for the same reason, and the lib's
        // `Array<T>` written by name is the mutable array itself.
        if reference.is_readonly_array() {
            match to {
                Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) | Type::Union(_) => return false,
                Type::Reference(target) if !target.is_readonly_array() => {
                    if target.arguments.len() == 1 && target.id.split('\u{0}').next_back() == Some("Array") {
                        return false;
                    }
                    return is_assignable_to(from, &target.resolve_arc());
                }
                _ => {}
            }
        }
        // relater.go: a pattern literal's base constraint is itself, so
        // against a primitive it relates by the simple rules alone — `string`
        // takes it and no literal does. Its `string` resolution would let the
        // comparable relation read `string` against a literal.
        if current_relation() == Relation::Comparable
            && (crate::is_template_literal_type(from) || crate::string_mapping_parts(from).is_some())
            && (matches!(
                to,
                Type::String
                    | Type::Number
                    | Type::Boolean
                    | Type::BigInt
                    | Type::Symbol
                    | Type::Undefined
                    | Type::Null
                    | Type::Void
                    | Type::StringLiteral(_)
                    | Type::NumberLiteral(_)
                    | Type::BooleanLiteral(_)
            ) || enum_member_value(to).is_some())
        {
            return is_simple_type_related_to(from, to);
        }
        // `resolve_arc` borrows the memoized/interned expansion instead of
        // deep-cloning it — this arm is peeled millions of times on
        // conditional-heavy programs.
        let resolved = reference.resolve_arc();
        return is_assignable_to(&resolved, to);
    }
    if let Type::Reference(reference) = to {
        // Any function (or callable/constructable object) is assignable to the
        // global `Function` interface. Its structural shape carries members a bare
        // function type does not expose (`prototype`, `arguments`, `caller`), so
        // the structural comparison below would wrongly reject it.
        let display = reference.display.as_ref();
        let base = display.split('<').next().unwrap_or(display);
        if base == "Function" && is_function_like(from) {
            return true;
        }
        // A primitive's apparent type *is* its global wrapper interface
        // (`getApparentType`), so `string` to `String` is an identity, not a
        // member-by-member comparison against a surface surge only partly
        // models.
        if reference.arguments.is_empty()
            && primitive_wrapper_interface(from) == Some(base)
            && is_global_wrapper_interface(reference, base)
        {
            return true;
        }
        let resolved = reference.resolve_arc();
        // A readonly target's apparent members are `ReadonlyArray<T>`'s, which
        // the mutable shape it resolves to would overstate.
        if reference.is_readonly_array()
            && let Type::Object(source) = from
        {
            return match resolved.as_ref() {
                Type::Array(element) => object_related_to_array_like(source, from, element, None, true),
                other => match crate::fixed_tuple_parts(other) {
                    Some(tuple) => object_related_to_array_like(
                        source,
                        from,
                        &crate::tuple_element_union(tuple.0),
                        Some(tuple),
                        true,
                    ),
                    None => false,
                },
            };
        }
        if let Some((elements, min_length)) = reference.written_tuple() {
            match from {
                Type::Tuple(source) => {
                    return tuple_related_to_fixed_tuple(
                        &fixed_tuple_elements(source),
                        &fixed_tuple_kinds(elements, min_length),
                    );
                }
                Type::Union(from_union) => return union_source_related(from_union, to),
                Type::Object(source) => {
                    return object_related_to_array_like(
                        source,
                        from,
                        &crate::tuple_element_union(elements),
                        Some((elements, min_length)),
                        false,
                    );
                }
                _ => {}
            }
        }
        return is_assignable_to(from, &resolved);
    }

    match (from, to) {
        (Type::Undefined, Type::Void) => true,
        (Type::Function(source), Type::Function(target)) => {
            is_function_assignable_to(source, target)
        }
        (Type::Array(source), Type::Array(target)) => is_assignable_to(source, target),
        // A source may stop short of trailing target slots that accept
        // `undefined` — how an optional element (`[string, string?]`) is
        // represented.
        (Type::Tuple(source), Type::Tuple(target)) => {
            source.len() <= target.len()
                && source
                    .iter()
                    .zip(target.iter())
                    .all(|(source_ty, target_ty)| is_assignable_to(source_ty, target_ty))
                && target[source.len()..]
                    .iter()
                    .all(|target_ty| is_assignable_to(&Type::Undefined, target_ty))
        }
        // relater.go relates a mutable tuple to an array through its number
        // index type, the union of its elements (`never` for `[]`).
        (Type::Tuple(source), Type::Array(target)) => match current_relation() {
            Relation::Assignable => source.iter().all(|source_ty| is_assignable_to(source_ty, target)),
            Relation::Comparable => {
                source.is_empty() || source.iter().any(|source_ty| is_assignable_to(source_ty, target))
            }
        },
        (Type::Tuple(source), Type::OpenTuple(target)) => {
            tuple_related_to_open_tuple(&fixed_tuple_elements(source), target)
        }
        (Type::OpenTuple(source), Type::OpenTuple(target)) => {
            tuple_related_to_open_tuple(&open_tuple_elements(source), target)
        }
        (Type::OpenTuple(source), Type::Array(target)) => {
            is_assignable_to(&source.element_union(), target)
        }
        // tsc refuses an array here (no guaranteed slots), but surge infers an
        // array literal as `T[]` wherever tsc would contextually type it as a
        // tuple, so refusing reports every `f([a, b])` against a `[T, ...T[]]`
        // parameter. Accept the array when its element fits every slot: the
        // arity check is deferred to the day literals are tuple-typed, and the
        // identity relation (`Equal`) still tells the two apart.
        (Type::Array(source), Type::OpenTuple(target)) => {
            target
                .leading
                .iter()
                .chain(target.trailing.iter())
                .chain(std::iter::once(target.rest.as_ref()))
                .all(|slot| is_assignable_to(source, slot))
        }
        // relater.go `structuredTypeRelatedToWorker`: an object that is not an
        // array or tuple reaches an array or fixed-tuple target through the
        // structural comparison, so `interface StrNum extends Array<string |
        // number> { 0: string; 1: number; length: 2 }` satisfies `[string,
        // number]`. A tuple with a rest element admits no such source
        // (`propertiesRelatedTo`'s `ElementFlagsVariable` check).
        (Type::Object(source), Type::Array(element)) => {
            object_related_to_array_like(source, from, element, None, false)
        }
        (Type::Object(source), Type::Tuple(elements)) => object_related_to_array_like(
            source,
            from,
            &crate::tuple_element_union(elements),
            Some((elements.as_slice(), crate::tuple_min_length(elements))),
            false,
        ),
        (Type::Union(from_union), Type::Union(_)) => {
            // Check each source member against the whole target union rather
            // than `any` single target member: a source member that is itself a
            // union (surge builds nested unions in a few synthesized spots)
            // fits the target member-wise, not as one atom.
            union_source_related(from_union, to)
        }
        (Type::Union(from_union), to_ty) => union_source_related(from_union, to_ty),
        (from_ty, Type::Union(to_union)) => {
            to_union
                .types()
                .iter()
                .any(|to_ty| is_assignable_to(from_ty, to_ty))
                || matches!(from_ty, Type::Object(_))
                    && discriminated_union_assignable(from_ty, to_union)
        }
        (Type::Object(from_obj), Type::Object(to_obj)) => {
            object_assignable(from_obj, to_obj, from, to)
        }
        // An object type carrying a call signature (e.g. `BooleanConstructor`,
        // or any `typeof fn` whose value also has properties) is assignable to a
        // function type when its call signature is. tsc treats such objects as
        // callable; without this an idiom like `arr.filter(Boolean)` is rejected.
        (Type::Object(source), Type::Function(target)) => source
            .call_signature()
            .is_some_and(|call_signature| is_function_assignable_to(call_signature, target)),
        // A function satisfies an object target when it matches the target's call
        // signature (if any) and supplies its required members. A callable interface
        // such as React's `ForwardRefRenderFunction` is the call-signature case; a
        // plain object whose members are all drawn from `Function.prototype` (`name`,
        // `length`, `call`/`apply`/`bind`, …) is the no-call-signature case — e.g. the
        // cross-realm `cls: {name: string}` idiom that accepts `typeof SomeClass`. A
        // construct-signature or index-signature target is left to the dedicated arms
        // above (or rejected), since a plain function value models neither.
        // An index signature of type `any` admits every object source,
        // functions included (tsc's `membersRelatedToIndexer` skips the members
        // for an `any` indexer); any other index type asks for an index the
        // function does not have.
        (Type::Function(source), Type::Object(target)) => {
            target.construct_signature().is_none()
                && target
                    .string_index_type
                    .as_deref()
                    .is_none_or(|index| matches!(index, Type::Any))
                && target
                    .number_index_type
                    .as_deref()
                    .is_none_or(|index| matches!(index, Type::Any))
                && match target.call_signature() {
                    Some(call_signature) => is_function_assignable_to(source, call_signature),
                    None => true,
                }
                && target.properties.iter().all(|(name, target_property)| {
                    match from.get_property_access_type(name) {
                        Some(source_ty) => is_assignable_to(&source_ty, &target_property.ty),
                        None => target_property.is_optional(),
                    }
                })
        }
        // A primitive structurally satisfies an object type that requires no
        // members — `{}`, all-optional shapes, and crucially the `T & {}` lib idiom
        // (e.g. `HTMLInputTypeAttribute = "button" | … | (string & {})`, where the
        // `string & {}` branch is what accepts an arbitrary `string`). tsc treats
        // any non-nullish value as assignable to such a type. Arrays and tuples are
        // objects too, so they likewise satisfy a no-required-member target — this
        // is what makes `Object.fromEntries(entries: [...][])` accept its argument
        // when the parameter degrades to `{}`.
        // The `object` keyword is the one no-member target a primitive does
        // not satisfy; arrays and tuples are objects and still do.
        (
            Type::String
            | Type::StringLiteral(_)
            | Type::Number
            | Type::NumberLiteral(_)
            | Type::Boolean
            | Type::BooleanLiteral(_)
            | Type::BigInt
            | Type::Symbol,
            Type::Object(target),
        ) if target.non_primitive => false,
        (
            Type::String
            | Type::StringLiteral(_)
            | Type::Number
            | Type::NumberLiteral(_)
            | Type::Boolean
            | Type::BooleanLiteral(_)
            | Type::BigInt
            | Type::Symbol
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::OpenTuple(_),
            Type::Object(target),
        ) => {
            // A required member is satisfied by whatever the source's own
            // intrinsic surface answers for it — `length` on an array or
            // string, `[Symbol.iterator]` on either. surge models these as
            // their own `Type` variants rather than as `Array`/`String`
            // interface instances, so without the lookup every lib interface an
            // array satisfies in tsc (`ArrayLike<T>`, `Iterable<T>`, a bare
            // `{ length: number }`) was rejected wholesale.
            target
                .properties
                .iter()
                .all(|(name, property)| match from.get_property_access_type(name) {
                    // A member declared with method syntax compares its
                    // parameters bivariantly, as it does between two objects:
                    // `ConcatArray<T>.join(separator?: string)` takes the
                    // array's own `join`.
                    Some(Type::Function(source_method)) if property.method => {
                        match property.ty.peeled() {
                            Type::Function(target_method) => {
                                is_signature_assignable_to(&source_method, &target_method, true)
                            }
                            _ => is_assignable_to(&Type::Function(source_method), &property.ty),
                        }
                    }
                    Some(source_ty) => is_assignable_to(&source_ty, &property.ty),
                    None => property.is_optional(),
                })
                // Only an index signature the source *declared* rejects here. A
                // checker-injected openness marker records an intersection operand
                // surge could not enumerate (`T & {}` where `T` stayed generic), so
                // treating it as a declared `[key: string]: T` turns surge's own
                // modelling loss into a false rejection of an array against `{}`.
                && !target.declares_string_index_access()
                && target.call_signature().is_none()
                && target.construct_signature().is_none()
        }
        _ => false,
    }
}

/// relater.go `templateLiteralTypesDefinitelyUnrelated`: the fixed texts
/// disagree where both templates start or where both end.
fn template_literal_types_definitely_unrelated(source_texts: &[&str], target_texts: &[&str]) -> bool {
    let (Some(source_start), Some(target_start), Some(source_end), Some(target_end)) = (
        source_texts.first().map(|text| text.as_bytes()),
        target_texts.first().map(|text| text.as_bytes()),
        source_texts.last().map(|text| text.as_bytes()),
        target_texts.last().map(|text| text.as_bytes()),
    ) else {
        return false;
    };
    let start = source_start.len().min(target_start.len());
    let end = source_end.len().min(target_end.len());
    source_start[..start] != target_start[..start]
        || source_end[source_end.len() - end..] != target_end[target_end.len() - end..]
}

/// Whether `ty` is a function or an object carrying a call/construct signature —
/// i.e. something assignable to the global `Function` interface.
/// A union source is related when *every* constituent is under the assignable
/// relation, but when *some* constituent is under the comparable one
/// (`relater.go`: `eachTypeRelatedToType` vs `someTypeRelatedToType`). This is
/// the only place the two relations diverge, and it applies at every level of a
/// structural comparison — which is why `[string, string | undefined]` overlaps
/// `string[]` even though it is not assignable to it.
fn union_source_related(from_union: &crate::UnionType, to: &Type) -> bool {
    // tsc's `containsType` shortcut: a source member that is itself a target
    // member relates without a comparison. Probing each one against every
    // target member instead costs n·m relation steps, which on two large
    // literal unions exhausts `MAX_ASSIGNABILITY_STEPS` and answers `true`.
    let target_index = match to {
        Type::Union(to_union) => crate::union::UnionMemberIndex::for_large(to_union.types()),
        _ => None,
    };
    let related = |from_ty: &Type| {
        target_index.as_ref().is_some_and(|index| index.contains(from_ty))
            || is_assignable_to(from_ty, to)
    };
    let mut members = from_union.types().iter();
    match current_relation() {
        Relation::Assignable => members.all(related),
        Relation::Comparable => members.any(related),
    }
}

/// The global interface a primitive's apparent type is.
fn primitive_wrapper_interface(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::String | Type::StringLiteral(_) => Some("String"),
        Type::Number | Type::NumberLiteral(_) => Some("Number"),
        Type::Boolean | Type::BooleanLiteral(_) => Some("Boolean"),
        Type::Symbol => Some("Symbol"),
        Type::BigInt => Some("BigInt"),
        _ => None,
    }
}

/// Whether `reference` names the lib's own wrapper interface rather than a
/// user type that happens to share the name: the lib's carries `valueOf`.
fn is_global_wrapper_interface(reference: &crate::TypeReference, name: &str) -> bool {
    reference.id.split('\u{0}').next_back() == Some(name)
        && matches!(
            &*reference.resolve_arc(),
            Type::Object(object) if object.properties.contains_key("valueOf")
        )
}

fn is_function_like(ty: &Type) -> bool {
    match ty {
        Type::Function(_) => true,
        Type::Object(object) => {
            object.call_signature().is_some() || object.construct_signature().is_some()
        }
        _ => false,
    }
}

/// Whether a parameter type carries the `unknown` degradation sentinel (NOT the
/// `unknown` keyword, which is `GenuineUnknown`) in an argument, member, union
/// arm, or signature position. Depth-bounded and reference-arguments-only (no
/// peel), so cyclic library reference graphs cannot loop.
/// Whether a *declared* parameter type still carries holes surge could not
/// fill — an unsubstituted type parameter, the degradation sentinel, or a
/// reference to a generic declaration written without arguments, whose members
/// were therefore built from the declaration's own parameters. Such an
/// expectation cannot reject an argument: the mismatch describes the hole, not
/// the source.
pub fn parameter_type_is_degraded(ty: &Type) -> bool {
    parameter_carries_degraded_unknown(ty, 0)
}

fn parameter_carries_degraded_unknown(ty: &Type, depth: usize) -> bool {
    carries_degraded_unknown(ty, depth, true)
}

/// A signature's parameter compared with another's: a type variable of the
/// body being checked is a real type there, not a hole.
fn signature_parameter_carries_hole(ty: &Type) -> bool {
    carries_degraded_unknown(ty, 0, false)
}

fn carries_degraded_unknown(ty: &Type, depth: usize, active_variables_are_holes: bool) -> bool {
    if depth > 3 {
        return false;
    }
    let parameter_carries_degraded_unknown =
        |ty: &Type, depth: usize| carries_degraded_unknown(ty, depth, active_variables_are_holes);
    match ty {
        Type::TypeParameter(parameter) => active_variables_are_holes || !parameter.is_active_variable(),
        Type::Unknown | Type::ErrorType => true,
        Type::Reference(reference) => {
            reference
                .arguments
                .iter()
                .any(|argument| parameter_carries_degraded_unknown(argument, depth + 1))
                // A lazily-deferred instantiation may carry its unresolved holes
                // only in the expanded body (its `arguments` can be empty for an
                // un-substituted parameter, e.g. `SubmitEvent<T>` in a library
                // signature resolved outside its generic context). Peel one level;
                // the depth bound keeps cyclic reference graphs from looping.
                || parameter_carries_degraded_unknown(&ty.peeled(), depth + 1)
        }
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| parameter_carries_degraded_unknown(&property.ty, depth + 1))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(|index| parameter_carries_degraded_unknown(index, depth + 1))
        }
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| parameter_carries_degraded_unknown(member, depth + 1)),
        // A generic member signature carries the sentinel where its own type
        // parameters were erased; that is a bound name, not a hole (see the
        // same arm in the checker's return walker).
        Type::Function(function) if function.type_parameter_head().is_some() => false,
        Type::Function(function) => {
            function
                .parameters()
                .iter()
                .any(|parameter| parameter_carries_degraded_unknown(parameter, depth + 1))
                || parameter_carries_degraded_unknown(function.return_type(), depth + 1)
        }
        Type::Array(element) => parameter_carries_degraded_unknown(element, depth + 1),
        _ => false,
    }
}

/// A tuple-typed rest parameter (`(...args: [a: A, b?: B]) => R`) *is* a
/// positional parameter list in tsc, so it must compare against a plainly
/// declared `(a: A, b?: B) => R`. Expands that trailing tuple into its elements;
/// every other signature is returned unchanged.
fn expanded_signature(function: &FunctionType) -> (std::borrow::Cow<'_, [Type]>, usize, bool) {
    let parameters = function.parameters();
    if function.is_variadic()
        && let Some(rest) = parameters.last()
        && let Type::Tuple(elements) = rest_slot_shape(rest)
    {
        let leading = parameters.len() - 1;
        let mut expanded = parameters[..leading].to_vec();
        expanded.extend(elements.iter().cloned());
        let required = function.required_parameter_count().min(leading)
            + written_tuple_min_length(rest).unwrap_or_else(|| {
                elements
                    .iter()
                    .take_while(|element| !type_includes_undefined(element))
                    .count()
            });
        return (std::borrow::Cow::Owned(expanded), required, false);
    }
    (
        std::borrow::Cow::Borrowed(parameters),
        function.required_parameter_count(),
        function.is_variadic(),
    )
}

/// Adds `undefined` to every optional, non-rest parameter under
/// `strictNullChecks`. Parameters surge could not type are left alone.
fn with_parameter_optionality(
    parameters: std::borrow::Cow<'_, [Type]>,
    required: usize,
    variadic: bool,
) -> std::borrow::Cow<'_, [Type]> {
    let rest_index = variadic.then(|| parameters.len().saturating_sub(1));
    let optional = |index: usize, parameter: &Type| {
        index >= required
            && Some(index) != rest_index
            && !parameter.is_unmodelled()
            && !matches!(parameter, Type::Any)
            && !type_includes_undefined(parameter)
    };
    if !crate::strict_null_checks()
        || !parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| optional(index, parameter))
    {
        return parameters;
    }
    std::borrow::Cow::Owned(
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                if optional(index, parameter) {
                    crate::union_type(vec![parameter.clone(), Type::Undefined])
                } else {
                    parameter.clone()
                }
            })
            .collect(),
    )
}

/// The structural shape behind a rest slot. A rest annotation resolves to the
/// array or tuple it is written as, but written as a deferred alias
/// (`...args: Parameters<F>`, vitest's `MockParameters<T>`) it arrives as a
/// lazy reference, and matching the variant on it directly compared the whole
/// rest as one positional parameter — every such mock read as not assignable
/// to any callback. The peel happens here, at comparison time, and not when the
/// signature is built: a declared generic signature holds that reference bound
/// to a placeholder, and forcing its memo there froze the placeholder
/// expansion for every later instantiation.
fn rest_slot_shape(rest: &Type) -> Type {
    match rest {
        Type::Reference(_) => rest.peeled(),
        other => other.clone(),
    }
}

/// The recorded `minLength` of the written tuple a rest slot names, through
/// the references in front of it.
fn written_tuple_min_length(rest: &Type) -> Option<usize> {
    let Type::Reference(reference) = rest else {
        return None;
    };
    match reference.written_tuple() {
        Some((_, min_length)) => Some(min_length),
        None => written_tuple_min_length(&reference.resolve_arc()),
    }
}

/// Widens a variadic signature's parameter list so a positional comparison
/// reaches every slot the rest parameter covers. A rest parameter is stored as
/// the array it is written as (`...items: T[]` -> `T[]`), so comparing it
/// positionally against a plainly declared `(a: T, b: T) => …` needs the array
/// expanded into `width` copies of its element.
fn widen_variadic_parameters<'a>(
    parameters: std::borrow::Cow<'a, [Type]>,
    is_variadic: bool,
    width: usize,
) -> std::borrow::Cow<'a, [Type]> {
    if !is_variadic {
        return parameters;
    }
    let Some(rest) = parameters.last() else {
        return parameters;
    };
    let leading = parameters.len() - 1;
    let mut widened = parameters[..leading].to_vec();
    match rest_slot_shape(rest) {
        Type::Array(element) => widened.resize(width.max(leading + 1), *element),
        // tsc's `getTypeAtPosition`: a rest parameter that is not itself a
        // tuple is indexed at each position, which distributes over a union of
        // tuples; a fixed tuple too short for the position reads `undefined`.
        Type::Union(union)
            if union
                .types()
                .iter()
                .all(|member| matches!(member.peeled(), Type::Tuple(_) | Type::OpenTuple(_))) =>
        {
            for position in 0..width.max(leading + 1) - leading {
                let elements = union
                    .types()
                    .iter()
                    .map(|member| match member.peeled() {
                        Type::Tuple(elements) => {
                            elements.get(position).cloned().unwrap_or(Type::Undefined)
                        }
                        Type::OpenTuple(open) => match open.leading.get(position) {
                            Some(element) => element.clone(),
                            None => {
                                let mut tail = vec![open.rest.as_ref().clone()];
                                tail.extend(open.trailing.iter().cloned());
                                crate::union_type(tail)
                            }
                        },
                        _ => unreachable!("every member is a tuple"),
                    })
                    .collect();
                widened.push(crate::union_type(elements));
            }
        }
        _ => return parameters,
    }
    std::borrow::Cow::Owned(widened)
}

fn type_includes_undefined(ty: &Type) -> bool {
    match ty {
        Type::Undefined => true,
        Type::Union(union) => union.types().iter().any(type_includes_undefined),
        _ => false,
    }
}

/// Whether a parameter slot accepts `void`, which makes tsc treat it as
/// optional for arity purposes (`getMinArgumentCount` walks the trailing
/// parameters back while they accept `void`). This is what lets a
/// `Promise<void>` executor's `resolve` — `(value: void | PromiseLike<void>)
/// => void` — be stored in a `() => void` slot.
fn parameter_accepts_void(ty: &Type) -> bool {
    match ty {
        Type::Void => true,
        Type::Union(union) => union.types().iter().any(parameter_accepts_void),
        _ => false,
    }
}

/// `required` with the trailing run of `void`-accepting parameters dropped.
fn required_count_ignoring_trailing_void(parameters: &[Type], required: usize) -> usize {
    let mut required = required.min(parameters.len());
    while required > 0 && parameter_accepts_void(&parameters[required - 1]) {
        required -= 1;
    }
    required
}

fn is_function_assignable_to(source: &FunctionType, target: &FunctionType) -> bool {
    // relater.go `signaturesRelatedTo`: the comparable relation erases the
    // type parameters of both signatures instead of instantiating the source
    // in the target's context, and an unsubstituted placeholder already
    // relates like the `any` erasure leaves.
    if current_relation() == Relation::Comparable {
        return is_signature_assignable_to(source, target, false);
    }
    // `signaturesRelatedTo` relates an overload group member by member with
    // every signature's type parameters erased too; the fold surge holds in
    // its place is related the same erased way.
    if source.overloads().is_some() || target.overloads().is_some() {
        return is_signature_assignable_to(source, target, false);
    }
    // `compareSignaturesRelated`: a generic target keeps its own type
    // parameters (`getCanonicalSignature`), and a generic source is
    // instantiated in the context of that canonical signature.
    if let Some(canonical_target) = opaque_generic_target(target) {
        let instantiated = generic_source_in_context_of(source, &canonical_target);
        return is_signature_assignable_to(instantiated.as_ref().unwrap_or(source), &canonical_target, false);
    }
    if let Some(instantiated) = generic_source_in_context_of(source, target) {
        return is_signature_assignable_to(&instantiated, target, false);
    }
    is_signature_assignable_to(source, target, false)
}

/// tsc's `instantiateSignatureInContextOf`: a generic source compared with a
/// non-generic target is first instantiated with what the target's parameters
/// infer for its type parameters, so `<T>(x: T) => T[]` against
/// `(x: number) => string[]` compares `number[]` with `string[]` and fails.
/// surge's placeholders relate like `unknown`, which accepted every such pair.
/// A constrained parameter stays a placeholder: the head is display text and
/// carries no resolved constraint to fall back to when the inference misses it.
pub fn generic_source_in_context_of(source: &FunctionType, target: &FunctionType) -> Option<FunctionType> {
    if target.type_parameter_head().is_some() {
        return None;
    }
    let head = source.type_parameter_head()?;
    let names: Vec<String> = head
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && !segment.contains(' ') && !segment.contains('<'))
        .map(String::from)
        .collect();
    if names.is_empty() {
        return None;
    }
    let mut candidates: Vec<InferenceCandidates> = names
        .iter()
        .map(|name| InferenceCandidates {
            name: name.clone(),
            covariant: Vec::new(),
            contravariant: Vec::new(),
        })
        .collect();
    // `getTypeAtPosition` reads an optional parameter with its `undefined`.
    let with_optionality = |function: &FunctionType, index: usize, ty: Type| {
        if crate::strict_null_checks()
            && index >= function.required_parameter_count()
            && !(function.is_variadic() && index + 1 >= function.parameters().len())
            && !ty.is_unmodelled()
            && !type_includes_undefined(&ty)
        {
            crate::union_type(vec![ty, Type::Undefined])
        } else {
            ty
        }
    };
    for index in 0..source.parameters().len() {
        let (Some(source_parameter), Some(target_parameter)) =
            (parameter_type_at(source, index), parameter_type_at(target, index))
        else {
            continue;
        };
        let source_parameter = with_optionality(source, index, source_parameter);
        let target_parameter = with_optionality(target, index, target_parameter);
        infer_to_type_parameters(&source_parameter, &target_parameter, &mut candidates, false, 0);
    }
    let inferred = |name: &str| -> Type {
        let Some(inference) = candidates.iter().find(|inference| inference.name == name) else {
            return Type::type_parameter(name);
        };
        // `getCovariantInference` widens literal candidates of a parameter the
        // return type does not expose at its top level.
        let widened: Vec<Type>;
        let covariant = if type_parameter_at_top_level(source.return_type(), name) {
            inference.covariant.as_slice()
        } else {
            widened = inference.covariant.iter().map(widen_literal_candidate).collect();
            widened.as_slice()
        };
        // The leftmost candidate every other one is assignable to, as
        // `getCommonSupertype` picks; with none, the first stands and the
        // comparison reports the disagreement.
        let supertype = covariant
            .iter()
            .find(|candidate| covariant.iter().all(|other| is_assignable_to(other, candidate)))
            .or_else(|| covariant.first());
        // `getCommonSubtype`: the leftmost candidate no later one is a subtype of.
        let subtype = inference.contravariant.iter().fold(None::<&Type>, |subtype, candidate| match subtype {
            Some(subtype) if !is_assignable_to(candidate, subtype) => Some(subtype),
            _ => Some(candidate),
        });
        // `getInferredType` keeps the covariant inference only when it fits
        // some contravariant candidate.
        let prefer_covariant = |supertype: &Type| {
            !matches!(supertype, Type::Never | Type::Any)
                && inference
                    .contravariant
                    .iter()
                    .any(|candidate| is_assignable_to(supertype, candidate))
        };
        match (supertype, subtype) {
            (Some(supertype), Some(subtype)) if !prefer_covariant(supertype) => subtype.clone(),
            (Some(inferred), _) | (None, Some(inferred)) => inferred.clone(),
            (None, None) => Type::type_parameter(name),
        }
    };
    let mut changed = false;
    let parameters: Vec<Type> = source
        .parameters()
        .iter()
        .map(|parameter| substitute_type_parameters(parameter, &names, &inferred, &mut changed))
        .collect();
    let return_type = substitute_type_parameters(source.return_type(), &names, &inferred, &mut changed);
    let resolved_any = candidates
        .iter()
        .any(|inference| !inference.covariant.is_empty() || !inference.contravariant.is_empty());
    (changed && resolved_any).then(|| {
        FunctionType::new(
            parameters,
            return_type,
            source.is_variadic(),
            source.required_parameter_count(),
        )
    })
}

/// `isTypeParameterAtTopLevel`: the type is the parameter or a union with it
/// as a member.
fn type_parameter_at_top_level(ty: &Type, name: &str) -> bool {
    match ty {
        Type::TypeParameter(parameter) => *parameter.name == *name && !ty.is_type_variable(),
        Type::Union(union) => union.types().iter().any(|member| type_parameter_at_top_level(member, name)),
        _ => false,
    }
}

/// `getWidenedLiteralType` of an inference candidate.
fn widen_literal_candidate(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Union(union) => crate::union_type(union.types().iter().map(widen_literal_candidate).collect()),
        other => other.clone(),
    }
}

/// tsc's `getTypeAtPosition`: a rest parameter answers for every position it
/// covers — its element for an array, the element at that offset for a tuple.
fn parameter_type_at(function: &FunctionType, index: usize) -> Option<Type> {
    let parameters = function.parameters();
    let last = parameters.len().checked_sub(1)?;
    if !function.is_variadic() || index < last {
        return parameters.get(index).cloned();
    }
    let offset = index - last;
    match parameters[last].peeled() {
        Type::Array(element) => Some(*element),
        Type::Tuple(elements) => elements.get(offset).cloned(),
        Type::OpenTuple(tuple) => Some(
            tuple
                .leading
                .get(offset)
                .cloned()
                .unwrap_or_else(|| tuple.rest.as_ref().clone()),
        ),
        other => Some(other),
    }
}

/// What one named type parameter of a generic source has been inferred from,
/// split by the variance of the position (`InferenceInfo.candidates` and
/// `contraCandidates`).
struct InferenceCandidates {
    name: String,
    covariant: Vec<Type>,
    contravariant: Vec<Type>,
}

/// Collects, for each named type parameter, the target types standing where
/// the source writes it. A parameter position of a nested signature flips the
/// variance (`inferFromContravariantTypesIfStrictFunctionTypes`).
fn infer_to_type_parameters(
    source: &Type,
    target: &Type,
    candidates: &mut Vec<InferenceCandidates>,
    contravariant: bool,
    depth: usize,
) {
    if depth > 8 {
        return;
    }
    match (source, target) {
        (Type::TypeParameter(parameter), target) if !source.is_type_variable() => {
            if target.is_unmodelled() || matches!(target, Type::Any) {
                return;
            }
            if let Some(inference) = candidates
                .iter_mut()
                .find(|inference| *inference.name == *parameter.name)
            {
                let types = if contravariant {
                    &mut inference.contravariant
                } else {
                    &mut inference.covariant
                };
                if !types.contains(target) {
                    types.push(target.clone());
                }
            }
        }
        (Type::Array(source), Type::Array(target)) => {
            infer_to_type_parameters(source, target, candidates, contravariant, depth + 1);
        }
        (Type::Tuple(source), Type::Tuple(target)) => {
            for (source, target) in source.iter().zip(target) {
                infer_to_type_parameters(source, target, candidates, contravariant, depth + 1);
            }
        }
        // A nested signature that declares one of the names itself shadows it
        // (`getErasedSignature` leaves nothing there to infer to).
        (Type::Function(source), Type::Function(target))
            if !source
                .type_parameter_names()
                .iter()
                .any(|own| candidates.iter().any(|inference| inference.name == *own)) =>
        {
            for (source, target) in source.parameters().iter().zip(target.parameters()) {
                infer_to_type_parameters(source, target, candidates, !contravariant, depth + 1);
            }
            infer_to_type_parameters(source.return_type(), target.return_type(), candidates, contravariant, depth + 1);
        }
        (Type::Reference(source), Type::Reference(target))
            if source.id == target.id && source.arguments.len() == target.arguments.len() =>
        {
            for (source, target) in source.arguments.iter().zip(target.arguments.iter()) {
                infer_to_type_parameters(source, target, candidates, contravariant, depth + 1);
            }
        }
        (Type::Object(source), Type::Object(target)) => {
            infer_from_object_members(source, target, candidates, contravariant, depth + 1);
        }
        // `inferFromMatchingTypes`: members both unions share are matched
        // off, and what remains of the target infers to the source's naked
        // type parameter.
        (Type::Union(source_union), target) => {
            let target_members: Vec<Type> = match target {
                Type::Union(target_union) => target_union.types().to_vec(),
                other => vec![other.clone()],
            };
            let (naked, fixed): (Vec<&Type>, Vec<&Type>) = source_union
                .types()
                .iter()
                .partition(|member| matches!(member, Type::TypeParameter(_)) && !member.is_type_variable());
            let [naked] = naked.as_slice() else {
                return;
            };
            let remaining: Vec<Type> =
                target_members.into_iter().filter(|member| !fixed.iter().any(|fixed| *fixed == member)).collect();
            if !remaining.is_empty() {
                infer_to_type_parameters(naked, &crate::union_type(remaining), candidates, contravariant, depth + 1);
            }
        }
        _ => {}
    }
}

/// `inferFromObjectTypes` between two object types: same-named properties,
/// then call and construct signatures, then index signatures — unless each
/// declares a required property the other lacks (`typesDefinitelyUnrelated`).
fn infer_from_object_members(
    source: &ObjectType,
    target: &ObjectType,
    candidates: &mut Vec<InferenceCandidates>,
    contravariant: bool,
    depth: usize,
) {
    let lacks_required_member_of = |object: &ObjectType, other: &ObjectType| {
        other
            .properties
            .iter()
            .any(|(name, property)| !property.optional && !object.properties.contains_key(name.as_ref()))
    };
    if lacks_required_member_of(source, target) && lacks_required_member_of(target, source) {
        return;
    }
    // `getTypeOfSymbol` reads an optional property with its `undefined`.
    let property_type = |property: &crate::ObjectProperty| {
        if property.optional
            && crate::strict_null_checks()
            && !property.ty.is_unmodelled()
            && !type_includes_undefined(&property.ty)
        {
            crate::union_type(vec![property.ty.clone(), Type::Undefined])
        } else {
            property.ty.clone()
        }
    };
    for (name, source_property) in source.properties.iter() {
        if let Some(target_property) = target.properties.get(name.as_ref()) {
            infer_to_type_parameters(
                &property_type(source_property),
                &property_type(target_property),
                candidates,
                contravariant,
                depth,
            );
        }
    }
    for (source_signature, target_signature) in [
        (source.call_signature(), target.call_signature()),
        (source.construct_signature(), target.construct_signature()),
    ] {
        if let (Some(source_signature), Some(target_signature)) = (source_signature, target_signature) {
            infer_to_type_parameters(
                &Type::Function(source_signature.clone()),
                &Type::Function(target_signature.clone()),
                candidates,
                contravariant,
                depth,
            );
        }
    }
    if let (Some(source_index), Some(target_index)) =
        (source.string_index_type.as_deref(), target.string_index_type.as_deref())
    {
        infer_to_type_parameters(source_index, target_index, candidates, contravariant, depth);
    }
    if let (Some(source_index), Some(target_index)) =
        (source.number_index_type.as_deref(), target.applicable_index_type(true))
    {
        infer_to_type_parameters(source_index, target_index, candidates, contravariant, depth);
    }
}

/// tsc's `getCanonicalSignature`: a generic target keeps its type parameters
/// as they are, whatever the source: `T` in `<T>(x: T) => T[]` is a type of
/// its own that `number` does not satisfy, so neither `(x: number) =>
/// number[]` nor `<U>(x: U) => string[]` is assignable to it. surge's
/// type-parameter placeholders relate like `unknown`, so for this comparison
/// each of the target's parameters is replaced with an opaque type only it
/// can inhabit. Constrained parameters are left alone: the head is display
/// text and does not carry a resolved constraint to relate through.
fn opaque_generic_target(target: &FunctionType) -> Option<FunctionType> {
    let head = target.type_parameter_head()?;
    let mut names = Vec::new();
    for segment in head.split(',') {
        let segment = segment.trim();
        if segment.is_empty() || segment.contains(' ') || segment.contains('<') {
            return None;
        }
        names.push(segment.to_string());
    }
    let opaque = |name: &str| {
        let mut properties = crate::PropertyMap::default();
        properties.insert(
            format!("\u{0}type parameter {name}").into(),
            crate::ObjectProperty::required(Type::Never),
        );
        Type::Object(ObjectType::new(properties, None))
    };
    let mut changed = false;
    let substitute = |ty: &Type, changed: &mut bool| substitute_type_parameters(ty, &names, &opaque, changed);
    let parameters: Vec<Type> = target
        .parameters()
        .iter()
        .map(|parameter| substitute(parameter, &mut changed))
        .collect();
    let return_type = substitute(target.return_type(), &mut changed);
    changed.then(|| {
        FunctionType::new(
            parameters,
            return_type,
            target.is_variadic(),
            target.required_parameter_count(),
        )
    })
}

/// `getSignatureInstantiation` for a signature whose own type parameters are
/// known only by name: each placeholder named in `names` is replaced by the
/// type at the same position.
pub fn instantiate_named_type_parameters(function: &FunctionType, names: &[String], types: &[Type]) -> FunctionType {
    // The signature's own parameters shadow any outer one of the same name,
    // bound as a type variable or not.
    fn replace(ty: &Type, names: &[String], types: &[Type]) -> Type {
        match ty {
            Type::TypeParameter(parameter) => names
                .iter()
                .position(|name| **name == *parameter.name)
                .and_then(|index| types.get(index).cloned())
                .unwrap_or_else(|| ty.clone()),
            Type::Array(element) => Type::Array(Box::new(replace(element, names, types))),
            Type::Tuple(elements) => Type::Tuple(elements.iter().map(|element| replace(element, names, types)).collect()),
            Type::Union(union) => crate::union_type(union.types().iter().map(|member| replace(member, names, types)).collect()),
            Type::Function(function) => Type::Function(replace_in_signature(function, names, types)),
            other => other.clone(),
        }
    }
    fn replace_in_signature(function: &FunctionType, names: &[String], types: &[Type]) -> FunctionType {
        FunctionType::new(
            function.parameters().iter().map(|parameter| replace(parameter, names, types)).collect(),
            replace(function.return_type(), names, types),
            function.is_variadic(),
            function.required_parameter_count(),
        )
    }
    replace_in_signature(function, names, types)
}

fn substitute_type_parameters(
    ty: &Type,
    names: &[String],
    opaque: &dyn Fn(&str) -> Type,
    changed: &mut bool,
) -> Type {
    match ty {
        Type::TypeParameter(parameter)
            if !ty.is_type_variable() && names.iter().any(|name| **name == *parameter.name) =>
        {
            *changed = true;
            opaque(&parameter.name)
        }
        Type::Array(element) => Type::Array(Box::new(substitute_type_parameters(element, names, opaque, changed))),
        Type::Tuple(elements) => Type::Tuple(
            elements
                .iter()
                .map(|element| substitute_type_parameters(element, names, opaque, changed))
                .collect(),
        ),
        Type::Union(union) => crate::union_type(
            union
                .types()
                .iter()
                .map(|member| substitute_type_parameters(member, names, opaque, changed))
                .collect(),
        ),
        Type::Function(function) => {
            // A nested generic signature's own type parameters shadow the
            // outer ones of the same name.
            let own = function.type_parameter_names();
            let unshadowed: Vec<String>;
            let names = if own.is_empty() {
                names
            } else {
                unshadowed = names.iter().filter(|name| !own.contains(name)).cloned().collect();
                unshadowed.as_slice()
            };
            Type::Function(FunctionType::new(
                function
                    .parameters()
                    .iter()
                    .map(|parameter| substitute_type_parameters(parameter, names, opaque, changed))
                    .collect(),
                substitute_type_parameters(function.return_type(), names, opaque, changed),
                function.is_variadic(),
                function.required_parameter_count(),
            ))
        }
        // A written object type (`{ a: T; b: T }`). An intersection surface,
        // an open one, a nominal declaration or the `object` keyword is left
        // as it is: rebuilding it here would drop what makes it that.
        Type::Object(object)
            if object.alias_id.is_none()
                && !object.is_intersection
                && !object.synthetic_open_index
                && !object.non_primitive
                && object.intersection_operands.is_none() =>
        {
            let mut object_changed = false;
            let properties: crate::PropertyMap = object
                .properties
                .iter()
                .map(|(name, property)| {
                    let mut property = property.clone();
                    property.ty = substitute_type_parameters(&property.ty, names, opaque, &mut object_changed);
                    (name.clone(), property)
                })
                .collect();
            let string_index = object
                .string_index_type
                .as_deref()
                .map(|index| substitute_type_parameters(index, names, opaque, &mut object_changed));
            let number_index = object
                .number_index_type
                .as_deref()
                .map(|index| substitute_type_parameters(index, names, opaque, &mut object_changed));
            if !object_changed {
                return ty.clone();
            }
            *changed = true;
            let mut substituted = ObjectType::new(properties, string_index).with_number_index_type(number_index);
            if object.without_inferable_index {
                substituted = substituted.with_nominal_declaration_marker();
            }
            if let Some(signature) = object.call_signature() {
                substituted = substituted.with_call_signature(signature.clone());
            }
            if let Some(signature) = object.construct_signature() {
                substituted = substituted.with_construct_signature(signature.clone());
            }
            Type::Object(substituted)
        }
        other => other.clone(),
    }
}

/// A member value against the member it fills: one declared with method
/// syntax compares its parameters bivariantly (`compareSignaturesRelated`
/// for a method declaration), even under `strictFunctionTypes`.
pub fn is_member_assignable_to(source: &Type, target: &Type, method: bool) -> bool {
    if method
        && let (Type::Function(source_signature), Type::Function(target_signature)) =
            (source.peeled(), target.peeled())
    {
        return is_signature_assignable_to(&source_signature, &target_signature, true);
    }
    is_assignable_to(source, target)
}

/// `bivariant_parameters` relaxes the contravariant parameter test to accept
/// either direction. tsc applies exactly that relaxation to members declared
/// with method syntax (`m(x: T): U`), even under `strictFunctionTypes` — an
/// override whose parameter is a *subtype* of the base's still satisfies it.
fn is_signature_assignable_to(
    source: &FunctionType,
    target: &FunctionType,
    bivariant_parameters: bool,
) -> bool {
    let (source_parameters, source_required, source_variadic) = expanded_signature(source);
    let (target_parameters, target_required, target_variadic) = expanded_signature(target);
    // tsc's `getTypeOfParameter`: an optional parameter's type includes
    // `undefined`, which is what the target passes when it omits the argument.
    // `(x: number) => void` therefore does not fit `(x?: number) => void`.
    let source_parameters =
        with_parameter_optionality(source_parameters, source_required, source_variadic);
    let target_parameters =
        with_parameter_optionality(target_parameters, target_required, target_variadic);
    let width = source_parameters.len().max(target_parameters.len());
    let source_parameters = widen_variadic_parameters(source_parameters, source_variadic, width);
    let target_parameters = widen_variadic_parameters(target_parameters, target_variadic, width);

    // A source function may declare fewer parameters than the target expects —
    // the surplus arguments the target would pass are simply ignored — but it
    // must not *require* more parameters than the target can ever supply. This
    // mirrors how tsc accepts `(v) => …` and `(v, i) => …` for an
    // `(element, index, array) => …` callback slot. The shared parameter prefix
    // is still checked bivariantly.
    let source_required = required_count_ignoring_trailing_void(&source_parameters, source_required);
    if !target_variadic && source_required > target_parameters.len() {
        return false;
    }

    let parameters_compatible = source_parameters.iter().zip(target_parameters.iter()).all(
        |(source_parameter, target_parameter)| {
            // A source parameter typed `unknown`/`any` accepts whatever argument
            // the target would supply, so it is contravariantly compatible with
            // any target parameter. This is what makes a generic call signature
            // whose unconstrained type parameter collapsed to `unknown` (e.g.
            // `BooleanConstructor`'s `<T>(value?: T) => boolean`) usable as a
            // typed callback such as an array predicate.
            // Contravariant, matching tsc under `strictFunctionTypes`: the
            // parameter the target would supply must be acceptable to the
            // source. Requiring covariance as well (the previous both-directions
            // rule) rejected a handler whose declared event type relates to the
            // slot's in only one direction, while pure covariance would wrongly
            // accept a literal-narrowed source (`(v: "idle") => void` as
            // `(v: string) => void`, TS2322 in tsc). A *degraded* target
            // parameter (one carrying the `unknown` sentinel in an argument or
            // member — e.g. a slot whose `KeyboardEvent<T>` kept an unresolved
            // `T`) cannot support the contravariant test (its `unknown` holes are
            // not assignable to the source's concrete members), so it falls back
            // to the covariant direction rather than flagging a handler tsc
            // accepts.
            source_parameter == target_parameter
                || source_parameter.is_unmodelled()
                || matches!(source_parameter, Type::Any)
                || is_assignable_to(target_parameter, source_parameter)
                || ((bivariant_parameters || signature_parameter_carries_hole(target_parameter))
                    && is_assignable_to(source_parameter, target_parameter))
        },
    );

    // A `void`-returning target ignores whatever the source returns: tsc accepts
    // any function as a `() => void` slot (`Array.prototype.forEach` callbacks,
    // event handlers, etc.). Outside that case the source return must be
    // assignable to the target's.
    let return_compatible = matches!(target.return_type(), Type::Void)
        || is_assignable_to(source.return_type(), target.return_type());

    parameters_compatible && return_compatible
}

/// tsc's `isWeakType`: an object whose members are all optional, with at
/// least one, and no call, construct or index signature.
pub fn is_weak_type(ty: &Type) -> bool {
    match ty.peeled() {
        Type::Object(object) => {
            !object.properties.is_empty()
                && object.properties.values().all(|property| property.optional)
                && object.string_index_type.is_none()
                && object.number_index_type.is_none()
                && object.call_signature.is_none()
                && object.construct_signature.is_none()
                && !object.synthetic_open_index
        }
        _ => false,
    }
}

/// The source of a weak-type failure whose first call or construct
/// signature returns something the target accepts: tsc then asks "Did you
/// mean to call it?" (TS2560).
pub fn weak_type_source_returns_target(source: &Type, target: &Type) -> bool {
    let signature = match source.peeled() {
        Type::Function(function) => Some(function),
        Type::Object(object) => object
            .call_signature()
            .or_else(|| object.construct_signature())
            .cloned(),
        _ => None,
    };
    signature.is_some_and(|signature| is_assignable_to(signature.return_type(), target))
}

/// The weak-type check of relater.go `isRelatedTo`: a source with members of
/// its own (or signatures), none of which the weak target declares, relates
/// to nothing but itself. A source surge did not model whole is not judged.
pub fn has_no_common_properties(source: &Type, target: &Type) -> bool {
    if !is_weak_type(target) || is_global_object_type(source) {
        return false;
    }
    let Type::Object(target) = target.peeled() else {
        return false;
    };
    match source.peeled() {
        Type::Object(source) => {
            !source.synthetic_open_index
                && !source.is_intersection
                && source.string_index_type.as_deref().is_none_or(|index| !index.is_unknown())
                && (!source.properties.is_empty()
                    || source.call_signature.is_some()
                    || source.construct_signature.is_some())
                && !source.properties.values().any(|property| property.ty.is_unknown())
                && !source.properties.keys().any(|name| target.properties.contains_key(name.as_ref()))
        }
        // A function type has signatures and no properties of its own.
        Type::Function(_) => true,
        // A primitive's properties are its apparent type's.
        primitive @ (Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)) => {
            !target.properties.keys().any(|name| primitive.get_property_access_type(name).is_some())
        }
        _ => false,
    }
}

/// tsc's `globalObjectType`, which `isRelatedTo` exempts from the
/// common-property check: the lib's `Object` interface.
fn is_global_object_type(ty: &Type) -> bool {
    const OBJECT_MEMBERS: [&str; 7] = [
        "constructor",
        "toString",
        "toLocaleString",
        "valueOf",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
    ];
    let named_object = match ty {
        Type::Reference(reference) => reference.id.split('\u{0}').next_back() == Some("Object"),
        Type::Object(object) => object.alias_name.as_deref() == Some("Object"),
        _ => false,
    };
    matches!(ty.peeled(), Type::Object(object)
        if OBJECT_MEMBERS.iter().all(|member| object.properties.contains_key(*member))
            && (named_object || object.properties.len() == OBJECT_MEMBERS.len()))
}

fn object_assignable(from_obj: &ObjectType, to_obj: &ObjectType, from: &Type, to: &Type) -> bool {
    let key = (
        Arc::as_ptr(&from_obj.properties) as usize,
        Arc::as_ptr(&to_obj.properties) as usize,
    );
    let newly_inserted = OBJECT_ASSIGNABILITY_IN_PROGRESS.with(|set| set.borrow_mut().insert(key));
    if !newly_inserted {
        record_assignability_assumption();
        return true;
    }

    let result = object_assignability_failure(from, to).is_none()
        && object_signatures_related(from_obj, to_obj)
        && index_signatures_related(from_obj, to_obj);
    OBJECT_ASSIGNABILITY_IN_PROGRESS.with(|set| {
        set.borrow_mut().remove(&key);
    });
    result
}

/// A tuple element's `ElementFlags`, as far as surge's shapes carry them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ElementKind {
    Required,
    Optional,
    Rest,
}

fn fixed_tuple_elements(elements: &[Type]) -> Vec<(ElementKind, &Type)> {
    fixed_tuple_kinds(elements, crate::tuple_min_length(elements))
}

fn fixed_tuple_kinds(elements: &[Type], min_length: usize) -> Vec<(ElementKind, &Type)> {
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let kind = if index < min_length { ElementKind::Required } else { ElementKind::Optional };
            (kind, element)
        })
        .collect()
}

/// A mutable tuple source against the tuple `to` names — through an alias,
/// and a readonly target takes it as it is — or `None` when `to` is no tuple.
fn tuple_source_related(source: &[(ElementKind, &Type)], to: &Type) -> Option<bool> {
    match to {
        Type::OpenTuple(target) => Some(tuple_related_to_open_tuple(source, target)),
        Type::Reference(reference) if reference.written_tuple().is_none() => {
            tuple_source_related(source, &reference.resolve_arc())
        }
        other => {
            let (elements, min_length) = crate::fixed_tuple_parts(other)?;
            Some(tuple_related_to_fixed_tuple(source, &fixed_tuple_kinds(elements, min_length)))
        }
    }
}

/// relater.go `propertiesRelatedTo` for a tuple source against a tuple target
/// without a rest element: the source must reach the target's `minLength` and
/// the target must hold the source's longest length, and each element is
/// related to its counterpart, which must not be required where the source's
/// may be missing.
fn tuple_related_to_fixed_tuple(source: &[(ElementKind, &Type)], target: &[(ElementKind, &Type)]) -> bool {
    let min_length = |elements: &[(ElementKind, &Type)]| {
        elements
            .iter()
            .filter(|(kind, _)| *kind == ElementKind::Required)
            .count()
    };
    let source_rest = source.iter().any(|(kind, _)| *kind == ElementKind::Rest);
    if !source_rest && source.len() < min_length(target)
        || target.len() < min_length(source)
        || source_rest
        || target.len() < source.len()
    {
        return false;
    }
    source
        .iter()
        .zip(target)
        .all(|((source_kind, source_type), (target_kind, target_type))| {
            (*target_kind != ElementKind::Required || *source_kind == ElementKind::Required)
                && is_assignable_to(source_type, target_type)
        })
}

fn open_tuple_elements(tuple: &crate::OpenTupleType) -> Vec<(ElementKind, &Type)> {
    let mut elements = fixed_tuple_elements(&tuple.leading);
    elements.push((ElementKind::Rest, tuple.rest.as_ref()));
    elements.extend(tuple.trailing.iter().map(|element| (ElementKind::Required, element)));
    elements
}

/// relater.go `propertiesRelatedTo` for a tuple source against a tuple target
/// with a rest element: a fixed source must reach the target's `minLength`,
/// and each source position is related to the target position it lands on —
/// counted from the start within the target's leading elements, from the end
/// past them — which must not be a required element the source may lack.
fn tuple_related_to_open_tuple(source: &[(ElementKind, &Type)], target: &crate::OpenTupleType) -> bool {
    let target_elements = open_tuple_elements(target);
    let target_arity = target_elements.len();
    let target_min_length = target_elements
        .iter()
        .filter(|(kind, _)| *kind == ElementKind::Required)
        .count();
    let source_rest = source.iter().any(|(kind, _)| *kind == ElementKind::Rest);
    if !source_rest && source.len() < target_min_length {
        return false;
    }
    let source_arity = source.len();
    source
        .iter()
        .enumerate()
        .all(|(source_position, (source_kind, source_type))| {
            let from_end = source_arity - 1 - source_position;
            let target_position = if source_position >= target.leading.len() {
                target_arity - 1 - from_end.min(target.trailing.len())
            } else {
                source_position
            };
            let (target_kind, target_type) = target_elements[target_position];
            (target_kind != ElementKind::Required || *source_kind == ElementKind::Required)
                && is_assignable_to(source_type, target_type)
        })
}

/// An object source against an array or fixed-tuple target, related as
/// against the object type the target's apparent type is (see
/// [`array_like_apparent_object`]). Most objects lack the array surface, so
/// the member names are checked before any member type is built.
fn object_related_to_array_like(
    source: &ObjectType,
    from: &Type,
    element: &Type,
    tuple: Option<(&[Type], usize)>,
    readonly: bool,
) -> bool {
    let has_array_members = crate::array_property_names()
        .iter()
        .filter(|name| !readonly || !crate::MUTATING_ARRAY_MEMBERS.contains(name))
        .all(|name| supplies_required_member(source, name));
    let has_elements = tuple.is_none_or(|(_, min_length)| {
        (0..min_length).all(|index| supplies_required_member(source, &index.to_string()))
    });
    if !has_array_members || !has_elements {
        return false;
    }
    let target = Type::Object(array_like_apparent_object(element, tuple, readonly));
    let Type::Object(target_object) = &target else {
        return false;
    };
    let related = object_assignable(source, target_object, from, &target);
    SYNTHESIZED_TARGETS.with(|targets| targets.borrow_mut().push(target));
    related
}

/// The members of an array's or a fixed tuple's apparent type: `Array<T>`'s
/// over the element type (`ReadonlyArray<T>`'s for a readonly one, which lacks
/// the mutators), and for a tuple the element properties and the `length`
/// `createTupleTargetType` declares, beside the number index signature.
fn array_like_apparent_object(element: &Type, tuple: Option<(&[Type], usize)>, readonly: bool) -> ObjectType {
    let mut properties = crate::PropertyMap::default();
    for name in crate::array_property_names() {
        if readonly && crate::MUTATING_ARRAY_MEMBERS.contains(name) {
            continue;
        }
        let property = if *name == "length" {
            let length = match tuple {
                Some((elements, min_length)) => crate::ty::tuple_length_literals(min_length, elements.len()),
                None => Type::Number,
            };
            crate::ObjectProperty::required(length)
        } else {
            let Some(member) = crate::array_member_type(name, element) else {
                continue;
            };
            crate::ObjectProperty::required(member).with_method(true)
        };
        properties.insert((*name).into(), property);
    }
    if let Some((elements, min_length)) = tuple {
        for (index, element) in elements.iter().enumerate() {
            let property = if index < min_length {
                crate::ObjectProperty::required(element.clone())
            } else {
                crate::ObjectProperty::optional(element.clone())
            };
            properties.insert(index.to_string().into(), property);
        }
    }
    // Built directly rather than through `ObjectType::new`: this shape lives
    // for one comparison and has no business in the canonical property-map
    // store.
    ObjectType {
        properties: Arc::new(properties),
        property_map_id: None,
        string_index_type: None,
        number_index_type: Some(Arc::new(element.clone())),
        alias_name: None,
        alias_id: None,
        construct_signature: None,
        call_signature: None,
        is_intersection: false,
        synthetic_open_index: false,
        non_primitive: false,
        without_inferable_index: false,
        intersection_operands: None,
    }
}

/// Whether `source` answers a required target member `name` the way
/// [`object_assignability_failure`] looks it up: a property, an index
/// signature standing for members surge could not enumerate, the `Function`
/// surface of a callable object, or the global `Object` members.
fn supplies_required_member(source: &ObjectType, name: &str) -> bool {
    source.properties.contains_key(name)
        || source
            .applicable_index_type(crate::object::is_numeric_key(name))
            .is_some_and(|index| source.synthetic_open_index || index.is_unknown())
        || callable_object_function_member(source, name).is_some()
        || object_prototype_member(name).is_some()
}

/// relater.go `reportUnmatchedProperty` for an object source against a fixed
/// tuple target: the one member the source lacks, which TS2741 names. With
/// more than one missing, `tryElaborateArrayLikeErrors` declines to list them
/// for a source that is not an array, so the plain assignability head stands.
pub fn tuple_target_missing_property(source: &Type, elements: &[Type]) -> Option<String> {
    let Type::Object(source) = source else {
        return None;
    };
    // `shouldReportUnmatchedPropertyError`: a source that is only a signature
    // is not reported by its members.
    if source.properties.is_empty()
        && (source.call_signature().is_some() || source.construct_signature().is_some())
    {
        return None;
    }
    let target = array_like_apparent_object(
        &crate::tuple_element_union(elements),
        Some((elements, crate::tuple_min_length(elements))),
        false,
    );
    let mut missing = target
        .required_properties()
        .filter(|(name, _)| !supplies_required_member(source, name))
        .map(|(name, _)| name.to_string());
    let first = missing.next()?;
    missing.next().is_none().then_some(first)
}

/// tsc's `indexSignaturesRelatedTo`. A target index signature is satisfied by
/// the source's applicable one, or — the source having none — by every source
/// member it would cover (the implicit index signature of an object type).
/// An `any`-valued signature asks for nothing once the target has a string
/// index at all. Only an object or type literal has that implicit signature
/// (`isObjectTypeWithInferableIndex`): an interface, a class instance or a
/// callable object answers with a signature it declares, or not at all.
/// A type variable of the body being checked is a type here like anywhere
/// else; only what surge could not model relates unconditionally.
fn index_signatures_related(source: &ObjectType, target: &ObjectType) -> bool {
    if target.synthetic_open_index || source.synthetic_open_index {
        return true;
    }
    let target_has_string_index = target.string_index_type.is_some();
    let exempt = |value: &Type| target_has_string_index && matches!(value, Type::Any);
    let related = |value: &Type, numeric_only: bool| {
        if exempt(value) || value.is_unmodelled() {
            return true;
        }
        if let Some(source_index) = source.applicable_index_type(numeric_only) {
            return source_index.is_unmodelled() || is_assignable_to(source_index, value);
        }
        if source.without_inferable_index
            || source.call_signature().is_some()
            || source.construct_signature().is_some()
        {
            return false;
        }
        // With no signature of the target's own kind, a numeric one still
        // covers keys a string signature answers (`membersRelatedToIndexer`).
        if !numeric_only
            && let Some(number_index) = source.number_index_type.as_deref()
            && !number_index.is_unmodelled()
            && !is_assignable_to(number_index, value)
        {
            return false;
        }
        source
            .properties
            .iter()
            .filter(|(name, _)| !numeric_only || crate::object::is_numeric_key(name.as_ref()))
            .all(|(_, property)| property.ty.is_unmodelled() || is_assignable_to(&property.ty, value))
    };
    target
        .string_index_type
        .as_deref()
        .is_none_or(|value| related(value, false))
        && target
            .number_index_type
            .as_deref()
            .is_none_or(|value| related(value, true))
}

/// tsc's `signaturesRelatedTo`, for both kinds: every call (construct)
/// signature the target declares has to be matched by one of the source's. A
/// target with none asks for nothing; a source with none matches nothing.
fn object_signatures_related(source: &ObjectType, target: &ObjectType) -> bool {
    signatures_of_kind_related(source.call_signature(), target.call_signature())
        && signatures_of_kind_related(source.construct_signature(), target.construct_signature())
}

fn signatures_of_kind_related(source: Option<&FunctionType>, target: Option<&FunctionType>) -> bool {
    let Some(target) = target else {
        return true;
    };
    let Some(source) = source else {
        return false;
    };
    let overloads = |signature: &FunctionType| -> Vec<FunctionType> {
        match signature.overloads() {
            Some(members) if !members.is_empty() => members.to_vec(),
            _ => vec![signature.clone()],
        }
    };
    let sources = overloads(source);
    let targets = overloads(target);
    // A member surge could not type says nothing about the group.
    let unmodelled = |signature: &FunctionType| {
        signature.parameters().iter().any(|parameter| matches!(parameter, Type::Unknown))
            || matches!(signature.return_type(), Type::Unknown)
    };
    if sources.iter().any(unmodelled) || targets.iter().any(unmodelled) {
        return true;
    }
    // relater.go `signaturesRelatedTo`: a single pair instantiates a generic
    // source in the target's context; with an overload group on either side
    // every signature's type parameters are erased, and each target member
    // needs some source member.
    if let ([source_signature], [target_signature]) = (sources.as_slice(), targets.as_slice()) {
        return is_function_assignable_to(source_signature, target_signature);
    }
    targets.iter().all(|target_signature| {
        sources
            .iter()
            .any(|source_signature| is_signature_assignable_to(source_signature, target_signature, false))
    })
}

/// Function/constructor objects (those carrying a call or construct signature)
/// also expose `Function.prototype` members. When such a source object lacks an
/// explicit property, fall back to these so a `typeof SomeClass` value satisfies
/// targets like `{name: string}`. Mirrors `function_property_access_type` in
/// `ty.rs`, but keyed off the object's call signature for `call`/`apply`/`bind`.
/// tsc's `getPropertyOfType` falls back to the global `Object` type's members
/// for every object type, so `{}` has `toString` and is assignable to
/// `Object`. The members' lib signatures, with `valueOf`'s `Object` as the
/// empty object type (no primitive, and it has these same members) and
/// `constructor`'s `Function` left as `any`.
fn object_prototype_member(name: &str) -> Option<Type> {
    let method = |parameters: Vec<Type>, return_type: Type| {
        let required = parameters.len();
        Some(Type::Function(FunctionType::new(parameters, return_type, false, required)))
    };
    match name {
        "toString" | "toLocaleString" => method(vec![], Type::String),
        "valueOf" => method(vec![], Type::Object(ObjectType::new(Default::default(), None))),
        "hasOwnProperty" | "propertyIsEnumerable" | "isPrototypeOf" => {
            method(vec![Type::Any], Type::Boolean)
        }
        "constructor" => Some(Type::Any),
        _ => None,
    }
}

fn callable_object_function_member(source: &ObjectType, name: &str) -> Option<Type> {
    let signature = source
        .call_signature()
        .or_else(|| source.construct_signature())?;
    match name {
        "length" => Some(Type::Number),
        "name" => Some(Type::String),
        "toString" | "toLocaleString" => Some(Type::Function(FunctionType::new(
            vec![],
            Type::String,
            false,
            0,
        ))),
        "call" | "apply" => Some(Type::Function(FunctionType::new(
            vec![],
            signature.return_type().clone(),
            true,
            0,
        ))),
        "bind" => Some(Type::Function(FunctionType::new(
            vec![],
            Type::Function(signature.clone()),
            true,
            0,
        ))),
        _ => None,
    }
}

/// Drops the `undefined` member a value may carry when the target property is
/// optional. `None` means the value was *only* `undefined`, which such a target
/// always accepts.
fn strip_undefined_member(ty: &Type) -> Option<Type> {
    match ty {
        Type::Undefined => None,
        Type::Union(union)
            if union
                .types()
                .iter()
                .any(|member| *member == Type::Undefined) =>
        {
            let kept: Vec<Type> = union
                .types()
                .iter()
                .filter(|member| **member != Type::Undefined)
                .cloned()
                .collect();
            if kept.is_empty() {
                None
            } else {
                Some(crate::union_type(kept))
            }
        }
        _ => Some(ty.clone()),
    }
}

/// relater.go `propertyRelatedTo` under the comparable relation. Both members
/// are read with their optionality (`getNonMissingTypeOfSymbol`), so two
/// optional members always overlap in `undefined`, and an optional source is
/// not held to a required target (`skipOptional`). A method target still
/// compares its parameters bivariantly.
fn comparable_property_related(source_ty: &Type, source_optional: bool, target: &crate::ObjectProperty) -> bool {
    let effective_source = with_optionality(source_ty, source_optional);
    let effective_target = with_optionality(&target.ty, target.is_optional());
    if target.is_method()
        && let (Type::Function(source_signature), Type::Function(target_signature)) = (source_ty, &target.ty)
    {
        return type_includes_undefined(&effective_source) && type_includes_undefined(&effective_target)
            || is_signature_assignable_to(source_signature, target_signature, true);
    }
    is_assignable_to(&effective_source, &effective_target)
}

/// An optional member's declared type as `getTypeOfSymbol` reads it: with
/// `undefined` under `strictNullChecks`.
fn with_optionality(ty: &Type, optional: bool) -> Type {
    if optional && crate::strict_null_checks() && !type_includes_undefined(ty) {
        crate::union_type(vec![ty.clone(), Type::Undefined])
    } else {
        ty.clone()
    }
}

/// tsc's `propertyRelatedTo` modifier rules. A private member on either side
/// relates only to the same declaration. A protected target needs a protected
/// source; tsc also requires the source's class to derive from the target's,
/// which the member alone cannot tell, so any protected source is accepted.
/// A protected source never relates to a public target.
fn restrictions_relate(
    source: Option<&crate::MemberRestriction>,
    target: Option<&crate::MemberRestriction>,
) -> bool {
    match (source, target) {
        (None, None) => true,
        (Some(source), Some(target)) if source.private || target.private => source == target,
        (Some(_), Some(_)) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

pub fn object_assignability_failure(
    source: &Type,
    target: &Type,
) -> Option<ObjectAssignabilityFailure> {
    let (Type::Object(source), Type::Object(target)) = (source, target) else {
        return None;
    };
    let comparable = current_relation() == Relation::Comparable;

    for (property_name, target_property) in target.properties.iter() {
        let source_property = source.properties.get(property_name.as_ref());
        // A declared index signature supplies no target property (tsc's
        // `propertiesRelatedTo` reads `getPropertyOfType` only): `{ [k: string]:
        // any }` is missing `hello` from `{ hello: string }`, and an optional
        // target property the source does not declare has nothing to relate.
        // Checker-injected openness still answers, since it stands for members
        // surge could not enumerate — and so does an `any`-valued signature,
        // which is also how surge spells an object it could not model (the
        // stand-in a generic body's own type parameter is evaluated with).
        let source_property_ty = source_property.map(|property| &property.ty).or_else(|| {
            let index = source
                .applicable_index_type(crate::object::is_numeric_key(property_name.as_ref()))?;
            (source.synthetic_open_index || index.is_unknown()).then_some(index)
        });

        let source_property_ty = source_property_ty
            .cloned()
            .or_else(|| callable_object_function_member(source, property_name.as_ref()))
            .or_else(|| object_prototype_member(property_name.as_ref()));

        let Some(source_property_ty) = source_property_ty.as_ref() else {
            // `getUnmatchedProperties` passes over a static private name the
            // source lacks.
            if target_property.is_optional() || crate::private_name::is_static(property_name) {
                continue;
            }

            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.to_string(),
            });
        };

        if let Some(source_property) = source_property
            && !restrictions_relate(
                source_property.restriction.as_ref(),
                target_property.restriction.as_ref(),
            )
        {
            return Some(ObjectAssignabilityFailure::AccessibilityMismatch {
                property_name: property_name.to_string(),
            });
        }

        if comparable {
            let source_optional = source_property.is_some_and(|property| property.is_optional());
            if !comparable_property_related(source_property_ty, source_optional, target_property) {
                return Some(ObjectAssignabilityFailure::PropertyTypeMismatch {
                    property_name: property_name.to_string(),
                    source_type: source_property_ty.clone(),
                    target_type: target_property.ty.clone(),
                });
            }
            continue;
        }

        if source_property.is_some()
            && source_property.is_some_and(|p| p.is_optional())
            && target_property.is_required()
        {
            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.to_string(),
            });
        }

        // Without `exactOptionalPropertyTypes` an optional target property accepts
        // an explicit `undefined`, so a required source property read as
        // `T | undefined` (typically itself an optional property's read type)
        // satisfies a `p?: T` target.
        let stripped_source_ty;
        let comparable_source_ty = if target_property.is_optional() {
            match strip_undefined_member(source_property_ty) {
                Some(stripped) => {
                    stripped_source_ty = stripped;
                    &stripped_source_ty
                }
                None => continue,
            }
        } else {
            source_property_ty
        };

        let property_assignable = match (
            target_property.is_method(),
            comparable_source_ty,
            &target_property.ty,
        ) {
            (true, Type::Function(source_signature), Type::Function(target_signature)) => {
                is_signature_assignable_to(source_signature, target_signature, true)
            }
            _ => is_assignable_to(comparable_source_ty, &target_property.ty),
        };

        if !property_assignable {
            return Some(ObjectAssignabilityFailure::PropertyTypeMismatch {
                property_name: property_name.to_string(),
                source_type: source_property_ty.clone(),
                target_type: target_property.ty.clone(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests;
