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
    static ASSIGNABILITY_RELATION_CACHE: std::cell::RefCell<crate::fx::FxHashMap<(RelationKey, RelationKey), bool>> =
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
}

const MAX_ASSIGNABILITY_DEPTH: u32 = 200;
const MAX_ASSIGNABILITY_STEPS: u64 = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RelationKey {
    tag: u8,
    parts: [usize; 5],
}

/// Stable identity for memoizing a comparison side. Every field that can change
/// the assignability verdict must contribute (properties, string index, call and
/// construct signatures, `alias_id` for the nominal fast path); types without a
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
            parts: [union.payload_address(), 0, 0, 0, 0],
        }),
        Type::Function(function) => Some(RelationKey {
            tag: 3,
            parts: [function.payload_address(), 0, 0, 0, 0],
        }),
        _ => None,
    }
}

fn record_assignability_assumption() {
    ASSIGNABILITY_ASSUMPTION_EVENTS.with(|events| events.set(events.get() + 1));
}

/// Widest source discriminant a distribution is attempted over, and widest
/// target union it is attempted against. Both are small in practice (a parse
/// status is three states); the caps keep a pathological union from turning one
/// failed comparison into a quadratic sweep.
const MAX_DISCRIMINANT_LITERALS: usize = 16;
const MAX_DISCRIMINATED_UNION_MEMBERS: usize = 32;

fn is_unit_literal(ty: &Type) -> bool {
    matches!(
        ty,
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_)
    )
}

/// tsc distributes an object over a union-typed discriminant before giving up on
/// a discriminated-union target: `{ status: "valid" | "dirty"; value: T }` is
/// assignable to `OK<T> | DIRTY<T>` because *each* status the source can carry
/// picks a member the rest of the object fits. Member-wise `any` misses that,
/// because the whole source matches no single member.
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

    // Nothing below runs unless the source carries a property that could *be* a
    // discriminant, so answer that from the source alone first. Peeling resolves
    // and clones each member's structural form, and this whole function runs on
    // every object-against-union comparison the member-wise check already
    // rejected — peeling up to 32 members before knowing there is a candidate
    // made the common "no discriminant here" answer the expensive one.
    if !from_object.properties.values().any(|property| {
        matches!(&property.ty, Type::Union(literals)
            if literals.types().len() <= MAX_DISCRIMINANT_LITERALS
                && literals.types().iter().all(is_unit_literal))
    }) {
        return false;
    }

    let peeled_members: Vec<Type> = members.iter().map(Type::peeled).collect();
    if !peeled_members
        .iter()
        .all(|member| matches!(member, Type::Object(_)))
    {
        return false;
    }

    for (property_name, property) in from_object.properties.iter() {
        let Type::Union(source_literals) = &property.ty else {
            continue;
        };
        if source_literals.types().len() > MAX_DISCRIMINANT_LITERALS
            || !source_literals.types().iter().all(is_unit_literal)
        {
            continue;
        }
        // Every target member must discriminate on this property for the
        // distribution to be sound.
        if !peeled_members.iter().all(|member| {
            member
                .get_property_access_type(property_name)
                .is_some_and(|ty| is_unit_literal(&ty))
        }) {
            continue;
        }

        if source_literals.types().iter().all(|literal| {
            let mut narrowed_properties = (*from_object.properties).clone();
            narrowed_properties.insert(
                property_name.clone(),
                crate::ObjectProperty {
                    ty: literal.clone(),
                    optional: property.optional,
                    method: property.method,
                },
            );
            let narrowed = Type::Object(ObjectType::new(
                narrowed_properties,
                from_object.string_index_type.as_deref().cloned(),
            ));
            peeled_members
                .iter()
                .any(|member| is_assignable_to(&narrowed, member))
        }) {
            return true;
        }
    }

    false
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
        || matches!(from, Type::Unknown | Type::TypeParameter(_))
    {
        return true;
    }

    // tsc lets any `number` flow into a numeric `enum` — its own
    // `Flags.A | Flags.B` is typed `number`, and the bitwise combination is the
    // normal way to build a flag argument. The reverse (a string into a string
    // enum) is rejected, which is why only the numeric marker opens this.
    if matches!(to, Type::Reference(reference) if reference.numeric_enum)
        && matches!(
            from.base_primitive().as_ref().unwrap_or(from),
            Type::Number | Type::NumberLiteral(_)
        )
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
            let pair = (from_key, to_key);
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

fn assignability_arms(from: &Type, to: &Type) -> bool {
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
                        matches!(from_arg, Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_))
                            || matches!(to_arg, Type::Any)
                            || is_assignable_to(from_arg, to_arg)
                    });
            if arguments_compatible {
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

    // Nominal references compare nominally first (same declaration + arguments is
    // handled by the `from == to` fast path above); anything else falls back to
    // comparing the structural expansion, so a reference stays interchangeable
    // with its expanded shape without forcing eager expansion at construction.
    if let Type::Reference(reference) = from {
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
        let resolved = reference.resolve_arc();
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
        (Type::Tuple(source), Type::Array(target)) => source
            .iter()
            .all(|source_ty| is_assignable_to(source_ty, target)),
        // A fixed tuple satisfies an open one when it covers the fixed slots on
        // both sides and everything in between fits the rest element.
        (Type::Tuple(source), Type::OpenTuple(target)) => {
            source.len() >= target.fixed_len()
                && source
                    .iter()
                    .zip(target.leading.iter())
                    .all(|(s, t)| is_assignable_to(s, t))
                && source[source.len() - target.trailing.len()..]
                    .iter()
                    .zip(target.trailing.iter())
                    .all(|(s, t)| is_assignable_to(s, t))
                && source[target.leading.len()..source.len() - target.trailing.len()]
                    .iter()
                    .all(|s| is_assignable_to(s, &target.rest))
        }
        (Type::OpenTuple(source), Type::OpenTuple(target)) => {
            source.leading.len() >= target.leading.len()
                && source.trailing.len() >= target.trailing.len()
                && source
                    .leading
                    .iter()
                    .zip(target.leading.iter())
                    .all(|(s, t)| is_assignable_to(s, t))
                && source.leading[target.leading.len()..]
                    .iter()
                    .all(|s| is_assignable_to(s, &target.rest))
                && is_assignable_to(&source.rest, &target.rest)
                && source.trailing[..source.trailing.len() - target.trailing.len()]
                    .iter()
                    .all(|s| is_assignable_to(s, &target.rest))
                && source.trailing[source.trailing.len() - target.trailing.len()..]
                    .iter()
                    .zip(target.trailing.iter())
                    .all(|(s, t)| is_assignable_to(s, t))
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
        (Type::Union(from_union), Type::Union(_)) => {
            // Check each source member against the whole target union rather
            // than `any` single target member: a source member that is itself a
            // union (surge builds nested unions in a few synthesized spots)
            // fits the target member-wise, not as one atom.
            from_union
                .types()
                .iter()
                .all(|from_ty| is_assignable_to(from_ty, to))
        }
        (Type::Union(from_union), to_ty) => from_union
            .types()
            .iter()
            .all(|from_ty| is_assignable_to(from_ty, to_ty)),
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
        (Type::Function(source), Type::Object(target)) => {
            target.construct_signature().is_none()
                && target.string_index_type.is_none()
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
            | Type::BooleanLiteral(_),
            Type::Object(target),
        ) if target.non_primitive => false,
        (
            Type::String
            | Type::StringLiteral(_)
            | Type::Number
            | Type::NumberLiteral(_)
            | Type::Boolean
            | Type::BooleanLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_)
            | Type::OpenTuple(_),
            Type::Object(target),
        ) => {
            target
                .properties
                .values()
                .all(|property| property.is_optional())
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

/// Whether `ty` is a function or an object carrying a call/construct signature —
/// i.e. something assignable to the global `Function` interface.
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
fn parameter_carries_degraded_unknown(ty: &Type, depth: usize) -> bool {
    if depth > 3 {
        return false;
    }
    match ty {
        Type::Unknown | Type::TypeParameter(_) => true,
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
            + elements
                .iter()
                .take_while(|element| !type_includes_undefined(element))
                .count();
        return (std::borrow::Cow::Owned(expanded), required, false);
    }
    (
        std::borrow::Cow::Borrowed(parameters),
        function.required_parameter_count(),
        function.is_variadic(),
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
    let Type::Array(element) = rest_slot_shape(rest) else {
        return parameters;
    };
    let leading = parameters.len() - 1;
    let element = *element;
    let mut widened = parameters[..leading].to_vec();
    widened.resize(width.max(leading + 1), element);
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
    is_signature_assignable_to(source, target, false)
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
    let (target_parameters, _, target_variadic) = expanded_signature(target);
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
                || source_parameter.is_unknown()
                || matches!(source_parameter, Type::Any)
                || is_assignable_to(target_parameter, source_parameter)
                || ((bivariant_parameters
                    || parameter_carries_degraded_unknown(target_parameter, 0))
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

    let result = object_assignability_failure(from, to).is_none();
    OBJECT_ASSIGNABILITY_IN_PROGRESS.with(|set| {
        set.borrow_mut().remove(&key);
    });
    result
}

/// Function/constructor objects (those carrying a call or construct signature)
/// also expose `Function.prototype` members. When such a source object lacks an
/// explicit property, fall back to these so a `typeof SomeClass` value satisfies
/// targets like `{name: string}`. Mirrors `function_property_access_type` in
/// `ty.rs`, but keyed off the object's call signature for `call`/`apply`/`bind`.
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

pub fn object_assignability_failure(
    source: &Type,
    target: &Type,
) -> Option<ObjectAssignabilityFailure> {
    let (Type::Object(source), Type::Object(target)) = (source, target) else {
        return None;
    };

    for (property_name, target_property) in target.properties.iter() {
        let source_property = source.properties.get(property_name.as_ref());
        let source_property_ty = source_property
            .map(|property| &property.ty)
            .or_else(|| source.string_index_type.as_deref());

        let source_property_ty = source_property_ty
            .cloned()
            .or_else(|| callable_object_function_member(source, property_name.as_ref()));

        let Some(source_property_ty) = source_property_ty.as_ref() else {
            if target_property.is_optional() {
                continue;
            }

            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.to_string(),
            });
        };

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
