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
    static OBJECT_ASSIGNABILITY_IN_PROGRESS: std::cell::RefCell<std::collections::HashSet<(usize, usize)>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

const MAX_ASSIGNABILITY_DEPTH: u32 = 200;

pub fn is_assignable_to(from: &Type, to: &Type) -> bool {
    struct DepthGuard;
    impl Drop for DepthGuard {
        fn drop(&mut self) {
            ASSIGNABILITY_DEPTH.with(|depth| {
                let next = depth.get().saturating_sub(1);
                depth.set(next);
                if next == 0 {
                    OBJECT_ASSIGNABILITY_IN_PROGRESS.with(|set| set.borrow_mut().clear());
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
        return true;
    }

    if from == to
        || matches!(from, Type::Any)
        || matches!(from, Type::Never)
        || matches!(to, Type::Any)
        || to.is_unknown()
    {
        return true;
    }

    if from
        .base_primitive()
        .as_ref()
        .is_some_and(|base| base == to)
    {
        return true;
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
                        matches!(from_arg, Type::Any | Type::Unknown | Type::GenuineUnknown)
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
        return is_assignable_to(&reference.resolve(), to);
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
        return is_assignable_to(from, &reference.resolve());
    }

    match (from, to) {
        (Type::Undefined, Type::Void) => true,
        (Type::Function(source), Type::Function(target)) => {
            is_function_assignable_to(source, target)
        }
        (Type::Array(source), Type::Array(target)) => is_assignable_to(source, target),
        (Type::Tuple(source), Type::Tuple(target)) => {
            source.len() == target.len()
                && source
                    .iter()
                    .zip(target.iter())
                    .all(|(source_ty, target_ty)| is_assignable_to(source_ty, target_ty))
        }
        (Type::Tuple(source), Type::Array(target)) => source
            .iter()
            .all(|source_ty| is_assignable_to(source_ty, target)),
        (Type::Union(from_union), Type::Union(to_union)) => {
            from_union.types().iter().all(|from_ty| {
                to_union
                    .types()
                    .iter()
                    .any(|to_ty| is_assignable_to(from_ty, to_ty))
            })
        }
        (Type::Union(from_union), to_ty) => from_union
            .types()
            .iter()
            .all(|from_ty| is_assignable_to(from_ty, to_ty)),
        (from_ty, Type::Union(to_union)) => to_union
            .types()
            .iter()
            .any(|to_ty| is_assignable_to(from_ty, to_ty)),
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
        (
            Type::String
            | Type::StringLiteral(_)
            | Type::Number
            | Type::NumberLiteral(_)
            | Type::Boolean
            | Type::BooleanLiteral(_)
            | Type::Array(_)
            | Type::Tuple(_),
            Type::Object(target),
        ) => {
            target
                .properties
                .values()
                .all(|property| property.is_optional())
                && target.string_index_type.is_none()
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
        Type::Unknown => true,
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

fn is_function_assignable_to(source: &FunctionType, target: &FunctionType) -> bool {
    // A source function may declare fewer parameters than the target expects —
    // the surplus arguments the target would pass are simply ignored — but it
    // must not *require* more parameters than the target can ever supply. This
    // mirrors how tsc accepts `(v) => …` and `(v, i) => …` for an
    // `(element, index, array) => …` callback slot. The shared parameter prefix
    // is still checked bivariantly.
    if !target.is_variadic() && source.required_parameter_count() > target.parameters().len() {
        return false;
    }

    let parameters_compatible = source
        .parameters()
        .iter()
        .zip(target.parameters().iter())
        .all(|(source_parameter, target_parameter)| {
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
                || (parameter_carries_degraded_unknown(target_parameter, 0)
                    && is_assignable_to(source_parameter, target_parameter))
        });

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

pub fn object_assignability_failure(
    source: &Type,
    target: &Type,
) -> Option<ObjectAssignabilityFailure> {
    let (Type::Object(source), Type::Object(target)) = (source, target) else {
        return None;
    };

    for (property_name, target_property) in target.properties.iter() {
        let source_property = source.properties.get(property_name.as_str());
        let source_property_ty = source_property
            .map(|property| &property.ty)
            .or_else(|| source.string_index_type.as_deref());

        let source_property_ty = source_property_ty
            .cloned()
            .or_else(|| callable_object_function_member(source, property_name.as_str()));

        let Some(source_property_ty) = source_property_ty.as_ref() else {
            if target_property.is_optional() {
                continue;
            }

            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.clone(),
            });
        };

        if source_property.is_some()
            && source_property.is_some_and(|p| p.is_optional())
            && target_property.is_required()
        {
            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.clone(),
            });
        }

        if !is_assignable_to(source_property_ty, &target_property.ty) {
            return Some(ObjectAssignabilityFailure::PropertyTypeMismatch {
                property_name: property_name.clone(),
                source_type: source_property_ty.clone(),
                target_type: target_property.ty.clone(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyMap;
    use crate::{FunctionType, NumberLiteralType, ObjectProperty, ObjectType, union_type};

    fn function_type(
        parameters: Vec<Type>,
        return_type: Type,
        is_variadic: bool,
        required_parameter_count: usize,
    ) -> Type {
        Type::Function(FunctionType::new(
            parameters,
            return_type,
            is_variadic,
            required_parameter_count,
        ))
    }

    fn name_target() -> Type {
        let mut properties = PropertyMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));
        Type::Object(ObjectType::new(properties, None))
    }

    #[test]
    fn function_with_any_return_assignable_to_void_returning_target() {
        // A `void`-returning target ignores the source return type, so a function
        // returning a value still satisfies it (e.g. an event handler slot).
        let source = function_type(vec![Type::Unknown], Type::Unknown, false, 1);
        let target = function_type(vec![Type::Unknown], Type::Void, false, 1);
        assert!(is_assignable_to(&source, &target));

        // The reverse does not hold: a non-`void` target still checks the return.
        let source_void = function_type(vec![Type::Unknown], Type::Void, false, 1);
        let target_number = function_type(vec![Type::Unknown], Type::Number, false, 1);
        assert!(!is_assignable_to(&source_void, &target_number));
    }

    #[test]
    fn function_assignable_to_name_object() {
        // A plain function value satisfies `{name: string}` via `Function.name`.
        let function = function_type(vec![], Type::Void, false, 0);
        assert!(is_assignable_to(&function, &name_target()));
    }

    #[test]
    fn function_not_assignable_to_object_with_extra_required_member() {
        let mut properties = PropertyMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));
        properties.insert("nope".to_string(), ObjectProperty::required(Type::Number));
        let target = Type::Object(ObjectType::new(properties, None));
        let function = function_type(vec![], Type::Void, false, 0);
        assert!(!is_assignable_to(&function, &target));
    }

    #[test]
    fn constructor_object_assignable_to_name_object() {
        // `typeof SomeClass` (a construct-signature object) satisfies
        // `{name: string}` via the synthesized `Function.name` member.
        let constructor =
            Type::Object(
                ObjectType::new(PropertyMap::new(), None)
                    .with_construct_signature(FunctionType::new(vec![], Type::Void, false, 0)),
            );
        assert!(is_assignable_to(&constructor, &name_target()));
    }

    #[test]
    fn function_assignable_to_function_interface() {
        // Any function is assignable to the global `Function` interface, even
        // though its structural shape (`prototype`/`arguments`/`caller`) is not
        // exposed by a bare function type. Also covers a `string | Function`
        // target (the DOM `setTimeout` handler).
        struct Resolver(Type);
        impl crate::ResolveReference for Resolver {
            fn resolve(&self) -> Type {
                self.0.clone()
            }
        }
        let mut members = PropertyMap::new();
        members.insert("prototype".to_string(), ObjectProperty::required(Type::Any));
        members.insert("arguments".to_string(), ObjectProperty::required(Type::Any));
        let function_interface = Type::Reference(crate::TypeReference::new(
            "lib.es5.d.ts\u{0}Function",
            "Function",
            Vec::new(),
            std::sync::Arc::new(Resolver(Type::Object(ObjectType::new(members, None)))),
        ));
        let function = function_type(vec![Type::Number], Type::Number, false, 1);
        assert!(is_assignable_to(&function, &function_interface));
        assert!(is_assignable_to(
            &function,
            &union_type(vec![Type::String, function_interface])
        ));
    }

    #[test]
    fn plain_object_without_name_not_assignable_to_name_object() {
        // The Function-member fallback must not leak to ordinary objects.
        let source = Type::Object(ObjectType::new(PropertyMap::new(), None));
        assert!(!is_assignable_to(&source, &name_target()));
    }

    #[test]
    fn array_assignable_to_empty_object() {
        // `Object.fromEntries([...])`: an array satisfies a no-required-member
        // object target (`{}`) just like any non-nullish value.
        let empty = Type::Object(ObjectType::new(PropertyMap::new(), None));
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::Number)),
            &empty
        ));
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Any]),
            &empty
        ));
    }

    #[test]
    fn array_not_assignable_to_object_with_required_member() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Number)),
            &name_target()
        ));
    }

    #[test]
    fn never_assignable_to_everything() {
        assert!(is_assignable_to(&Type::Never, &Type::String));
        assert!(is_assignable_to(&Type::Never, &Type::Number));
        assert!(is_assignable_to(&Type::Never, &Type::Never));
    }

    /// A reference whose structural expansion is keyed on its own display name,
    /// so any two distinct instantiations are structurally incompatible — a
    /// passing assertion proves the nominal argument path was taken rather than
    /// the structural fallback.
    fn opaque_generic(id: &str, display: &str, argument: Type) -> Type {
        struct Opaque(String);
        impl crate::ResolveReference for Opaque {
            fn resolve(&self) -> Type {
                let mut members = PropertyMap::new();
                members.insert(self.0.clone(), ObjectProperty::required(Type::Never));
                Type::Object(ObjectType::new(members, None))
            }
        }
        Type::Reference(crate::TypeReference::new(
            id,
            display,
            vec![argument],
            std::sync::Arc::new(Opaque(display.to_string())),
        ))
    }

    #[test]
    fn same_generic_declaration_compares_by_arguments_not_structure() {
        let set_any = opaque_generic("lib\u{0}Set", "Set<any>", Type::Any);
        let set_string = opaque_generic("lib\u{0}Set", "Set<string>", Type::String);
        let set_unknown = opaque_generic("lib\u{0}Set", "Set<unknown>", Type::Unknown);
        let set_number = opaque_generic("lib\u{0}Set", "Set<number>", Type::Number);

        assert!(is_assignable_to(&set_any, &set_string));
        assert!(is_assignable_to(&set_string, &set_any));
        assert!(is_assignable_to(&set_unknown, &set_string));
        assert!(!is_assignable_to(&set_string, &set_number));
    }

    #[test]
    fn different_generic_declarations_do_not_compare_by_arguments() {
        let set = opaque_generic("lib\u{0}Set", "Set<any>", Type::Any);
        let map = opaque_generic("lib\u{0}Map", "Map<any>", Type::Any);
        assert!(!is_assignable_to(&set, &map));
    }

    #[test]
    fn same_generic_reference_assignable_to_union_member() {
        let set_any = opaque_generic("lib\u{0}Set", "Set<any>", Type::Any);
        let set_string = opaque_generic("lib\u{0}Set", "Set<string>", Type::String);
        let target = union_type(vec![set_string, Type::Undefined]);
        assert!(is_assignable_to(&set_any, &target));
    }

    #[test]
    fn nothing_assignable_to_never_except_never() {
        assert!(!is_assignable_to(&Type::String, &Type::Never));
        assert!(!is_assignable_to(&Type::Undefined, &Type::Never));
    }

    #[test]
    fn string_literal_assignable_to_string() {
        assert!(is_assignable_to(
            &Type::StringLiteral("ok".to_string()),
            &Type::String
        ));
    }

    #[test]
    fn string_not_assignable_to_string_literal() {
        assert!(!is_assignable_to(
            &Type::String,
            &Type::StringLiteral("ok".to_string())
        ));
    }

    #[test]
    fn matching_string_literal_assignable_to_same_literal() {
        let literal = Type::StringLiteral("ok".to_string());
        assert!(is_assignable_to(&literal, &literal));
    }

    #[test]
    fn different_string_literal_not_assignable() {
        assert!(!is_assignable_to(
            &Type::StringLiteral("ok".to_string()),
            &Type::StringLiteral("no".to_string())
        ));
    }

    #[test]
    fn number_literal_assignability() {
        let literal = Type::NumberLiteral(NumberLiteralType {
            value: "1".to_string(),
        });

        assert!(is_assignable_to(&literal, &Type::Number));
        assert!(!is_assignable_to(
            &Type::Number,
            &Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            })
        ));
    }

    #[test]
    fn boolean_literal_assignability() {
        assert!(is_assignable_to(
            &Type::BooleanLiteral(true),
            &Type::Boolean
        ));
        assert!(!is_assignable_to(
            &Type::Boolean,
            &Type::BooleanLiteral(true)
        ));
        assert!(!is_assignable_to(
            &Type::BooleanLiteral(true),
            &Type::BooleanLiteral(false)
        ));
    }

    #[test]
    fn literal_union_assignability() {
        assert!(is_assignable_to(
            &union_type(vec![
                Type::StringLiteral("ok".to_string()),
                Type::StringLiteral("no".to_string())
            ]),
            &Type::String
        ));
        assert!(!is_assignable_to(
            &Type::String,
            &union_type(vec![
                Type::StringLiteral("ok".to_string()),
                Type::StringLiteral("no".to_string())
            ])
        ));
    }

    #[test]
    fn literal_object_assignability() {
        let mut source_properties = PropertyMap::new();
        source_properties.insert(
            "kind".to_string(),
            ObjectProperty::required(Type::StringLiteral("click".to_string())),
        );

        let mut target_properties = PropertyMap::new();
        target_properties.insert("kind".to_string(), ObjectProperty::required(Type::String));

        assert!(is_assignable_to(
            &Type::Object(ObjectType::new(source_properties, None)),
            &Type::Object(ObjectType::new(target_properties, None))
        ));
    }

    #[test]
    fn void_assignability() {
        assert!(is_assignable_to(&Type::Undefined, &Type::Void));
        assert!(is_assignable_to(&Type::Void, &Type::Any));
        assert!(is_assignable_to(&Type::Any, &Type::Void));
        assert!(!is_assignable_to(&Type::Void, &Type::Undefined));
    }

    #[test]
    fn undefined_assignable_to_void() {
        assert!(is_assignable_to(&Type::Undefined, &Type::Void));
    }

    #[test]
    fn void_assignable_to_void() {
        assert!(is_assignable_to(&Type::Void, &Type::Void));
    }

    #[test]
    fn any_assignable_to_void() {
        assert!(is_assignable_to(&Type::Any, &Type::Void));
    }

    #[test]
    fn void_assignable_to_any() {
        assert!(is_assignable_to(&Type::Void, &Type::Any));
    }

    #[test]
    fn void_not_assignable_to_undefined() {
        assert!(!is_assignable_to(&Type::Void, &Type::Undefined));
    }

    #[test]
    fn void_not_assignable_to_string() {
        assert!(!is_assignable_to(&Type::Void, &Type::String));
    }

    #[test]
    fn void_union_assignability() {
        let void_union = union_type(vec![Type::Void, Type::Undefined]);

        assert!(is_assignable_to(&Type::Undefined, &void_union));
        assert!(is_assignable_to(&Type::Void, &void_union));
        assert!(is_assignable_to(&void_union, &Type::Void));
        assert!(!is_assignable_to(&void_union, &Type::Undefined));
    }

    #[test]
    fn function_type_assignable_same_signature() {
        let source = function_type(vec![Type::String], Type::Number, false, 1);
        let target = function_type(vec![Type::String], Type::Number, false, 1);

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_assignable_fewer_source_parameters() {
        // A callback declaring fewer parameters than the target expects is
        // assignable: the surplus arguments are simply ignored (e.g. `(v) => …`
        // for an `(element, index) => …` slot).
        let source = function_type(vec![Type::String], Type::Number, false, 1);
        let target = function_type(vec![Type::String, Type::Number], Type::Number, false, 2);

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_more_required_source_parameters() {
        // The source requires two parameters but the target only ever supplies
        // one, so it cannot be called safely.
        let source = function_type(vec![Type::String, Type::Number], Type::Number, false, 2);
        let target = function_type(vec![Type::String], Type::Number, false, 1);

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_parameter_mismatch() {
        let source = function_type(vec![Type::Number], Type::Number, false, 1);
        let target = function_type(vec![Type::String], Type::Number, false, 1);

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_return_mismatch() {
        let source = function_type(vec![Type::String], Type::String, false, 1);
        let target = function_type(vec![Type::String], Type::Number, false, 1);

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_return_covariance_basic() {
        let source = function_type(
            vec![Type::String],
            Type::StringLiteral("ok".to_string()),
            false,
            1,
        );
        let target = function_type(vec![Type::String], Type::String, false, 1);

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_literal_parameter_conservative() {
        let source = function_type(
            vec![Type::StringLiteral("ok".to_string())],
            Type::Void,
            false,
            1,
        );
        let target = function_type(vec![Type::String], Type::Void, false, 1);

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_union_parameter_compatible_when_same() {
        let source = function_type(
            vec![union_type(vec![Type::String, Type::Number])],
            Type::Void,
            false,
            1,
        );
        let target = function_type(
            vec![union_type(vec![Type::String, Type::Number])],
            Type::Void,
            false,
            1,
        );

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_void_return_assignable_to_void_return() {
        let source = function_type(vec![Type::String], Type::Void, false, 1);
        let target = function_type(vec![Type::String], Type::Void, false, 1);

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_void_return_not_assignable_to_string_return() {
        let source = function_type(vec![Type::String], Type::Void, false, 1);
        let target = function_type(vec![Type::String], Type::String, false, 1);

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn array_type_name_string() {
        assert_eq!(Type::Array(Box::new(Type::String)).name(), "string[]");
    }

    #[test]
    fn array_type_name_number() {
        assert_eq!(Type::Array(Box::new(Type::Number)).name(), "number[]");
    }

    #[test]
    fn array_type_name_literal() {
        assert_eq!(
            Type::Array(Box::new(Type::StringLiteral("ok".to_string()))).name(),
            r#""ok"[]"#
        );
    }

    #[test]
    fn array_type_name_union() {
        assert_eq!(
            Type::Array(Box::new(union_type(vec![Type::String, Type::Number]))).name(),
            "(string | number)[]"
        );
    }

    #[test]
    fn array_type_name_function() {
        assert_eq!(
            Type::Array(Box::new(function_type(vec![], Type::String, false, 0))).name(),
            "(() => string)[]"
        );
    }

    #[test]
    fn array_assignable_same_element() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_assignable_literal_element_to_base_element() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::StringLiteral("ok".to_string()))),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_not_assignable_base_element_to_literal_element() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::StringLiteral("ok".to_string())))
        ));
    }

    #[test]
    fn array_not_assignable_different_element() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Number)),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_any_element_assignability() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::Any)),
            &Type::Array(Box::new(Type::String))
        ));
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::Any))
        ));
    }

    #[test]
    fn array_not_assignable_to_primitive() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::String
        ));
    }

    #[test]
    fn primitive_not_assignable_to_array() {
        assert!(!is_assignable_to(
            &Type::String,
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_union_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number]))),
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number])))
        ));
    }

    #[test]
    fn array_union_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Boolean)),
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number])))
        ));
    }

    #[test]
    fn array_function_element_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(function_type(vec![], Type::Void, false, 0))),
            &Type::Array(Box::new(function_type(vec![], Type::Void, false, 0)))
        ));
    }

    #[test]
    fn array_function_element_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(function_type(
                vec![Type::String],
                Type::Void,
                false,
                1
            ))),
            &Type::Array(Box::new(function_type(vec![], Type::Void, false, 0)))
        ));
    }

    #[test]
    fn tuple_type_name_empty_if_supported() {
        assert_eq!(Type::Tuple(vec![]).name(), "[]");
    }

    #[test]
    fn tuple_type_name_one_element() {
        assert_eq!(Type::Tuple(vec![Type::String]).name(), "[string]");
    }

    #[test]
    fn tuple_type_name_two_elements() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).name(),
            "[string, number]"
        );
    }

    #[test]
    fn tuple_type_name_literal_element() {
        assert_eq!(
            Type::Tuple(vec![Type::StringLiteral("ok".to_string()), Type::Number]).name(),
            r#"["ok", number]"#
        );
    }

    #[test]
    fn tuple_type_name_union_element() {
        assert_eq!(
            Type::Tuple(vec![
                union_type(vec![Type::String, Type::Number]),
                Type::Boolean
            ])
            .name(),
            "[string | number, boolean]"
        );
    }

    #[test]
    fn tuple_type_name_function_element() {
        assert_eq!(
            Type::Tuple(vec![
                function_type(vec![], Type::Void, false, 0),
                Type::String,
            ])
            .name(),
            "[() => void, string]"
        );
    }

    #[test]
    fn tuple_type_name_object_element() {
        let mut properties = PropertyMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Tuple(vec![
                Type::Object(ObjectType::new(properties, None)),
                Type::Number
            ])
            .name(),
            "[{ name: string; }, number]"
        );
    }

    #[test]
    fn tuple_type_name_array_element() {
        assert_eq!(
            Type::Tuple(vec![Type::Array(Box::new(Type::String)), Type::Number]).name(),
            "[string[], number]"
        );
    }

    #[test]
    fn tuple_type_name_nested_tuple() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Tuple(vec![Type::String, Type::Number]),
                Type::Boolean,
            ])
            .name(),
            "[[string, number], boolean]"
        );
    }

    #[test]
    fn tuple_assignable_same_shape() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_assignable_literal_to_base() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![
                Type::StringLiteral("ok".to_string()),
                Type::NumberLiteral(NumberLiteralType {
                    value: "1".to_string(),
                })
            ]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_not_assignable_base_to_literal() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![
                Type::StringLiteral("ok".to_string()),
                Type::NumberLiteral(NumberLiteralType {
                    value: "1".to_string(),
                })
            ])
        ));
    }

    #[test]
    fn tuple_not_assignable_different_length_too_short() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_not_assignable_different_length_too_long() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![Type::String])
        ));
    }

    #[test]
    fn tuple_not_assignable_element_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::String]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_assignable_to_array_when_elements_compatible() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::StringLiteral("ok".to_string())]),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn tuple_not_assignable_to_array_when_element_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_not_assignable_to_tuple() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Tuple(vec![Type::String, Type::String])
        ));
    }

    #[test]
    fn tuple_any_assignability() {
        assert!(is_assignable_to(
            &Type::Any,
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Any
        ));
    }

    #[test]
    fn tuple_union_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &union_type(vec![
                Type::Tuple(vec![Type::String, Type::Number]),
                Type::String
            ])
        ));
    }

    #[test]
    fn tuple_union_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &union_type(vec![Type::Tuple(vec![Type::String, Type::String])])
        ));
    }
}
