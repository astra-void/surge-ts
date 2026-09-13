use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{Type, TypeCopyReason, union_type};

use crate::symbols::{SymbolInfo, SymbolTable};

/// What a union member proves about `"prop" in value`.
pub(super) enum PropertyPresence {
    /// Declared and required: every value of the member has the key.
    Required,
    /// Declared optional: the key may or may not be there at runtime, so the
    /// member survives *both* branches (tsc keeps `{ a?: X }` in the else branch
    /// of `if ("a" in v)`).
    Optional,
    Absent,
    /// Not statically decidable (a string index signature, a non-object member).
    Undecidable,
}

pub(super) fn property_presence_of(member: &Type, property: &str) -> PropertyPresence {
    match member {
        Type::Object(object) => match object.get_property(property) {
            Some(existing) if existing.is_optional() => PropertyPresence::Optional,
            Some(_) => PropertyPresence::Required,
            None if object.allows_string_index_access() => PropertyPresence::Undecidable,
            None => PropertyPresence::Absent,
        },
        _ => PropertyPresence::Undecidable,
    }
}

pub(super) enum PresenceNarrowing {
    Kept,
    Removed,
    Narrowed(Type),
}

/// Decides a single union member. Named aliases and interfaces arrive as nominal
/// `Type::Reference` wrappers, and an alias to a union stays a nested union after
/// peeling — both must be looked through, or every member is "undecidable" and
/// the guard narrows nothing. The unpeeled member is what the caller keeps when
/// nothing was dropped, so nominal display names survive in diagnostics.
pub(super) fn narrow_member_by_property_presence(
    member: &Type,
    property: &str,
    keep_present: bool,
) -> PresenceNarrowing {
    let peeled = member.peeled();
    if let Type::Union(inner) = &peeled {
        let mut kept = Vec::new();
        let mut changed = false;
        for constituent in inner.types().iter() {
            match narrow_member_by_property_presence(constituent, property, keep_present) {
                PresenceNarrowing::Kept => kept.push(constituent.clone()),
                PresenceNarrowing::Removed => changed = true,
                PresenceNarrowing::Narrowed(narrowed) => {
                    changed = true;
                    kept.push(narrowed);
                }
            }
        }
        if !changed {
            return PresenceNarrowing::Kept;
        }
        if kept.is_empty() {
            return PresenceNarrowing::Removed;
        }
        return PresenceNarrowing::Narrowed(union_type(kept));
    }

    let survives = match property_presence_of(&peeled, property) {
        PropertyPresence::Required => keep_present,
        PropertyPresence::Absent => !keep_present,
        PropertyPresence::Optional | PropertyPresence::Undecidable => true,
    };
    if survives {
        PresenceNarrowing::Kept
    } else {
        PresenceNarrowing::Removed
    }
}

/// Narrows a union by whether each member has `property` (`"prop" in obj`).
/// `keep_present` selects members that have it (the `in` true branch).
/// Members every object answers through `Object.prototype`. `"toString" in x` is
/// true for any object, so it proves nothing new, and synthesizing the key would
/// shadow the real member and turn a working `x.toString()` into an error.
pub(super) fn is_object_prototype_member(property: &str) -> bool {
    matches!(
        property,
        "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "constructor"
            | "__proto__"
    )
}

pub(crate) fn narrow_union_by_property_presence(
    ty: &Type,
    property: &str,
    keep_present: bool,
) -> Option<Type> {
    let peeled = ty.peeled();
    // A non-union object learns the key it was just tested for. tsc reports the
    // result as `T & Record<"p", unknown>`; carrying the property on the object
    // itself is the same member surface. Only the true branch learns anything —
    // absence proves nothing about a type that never declared the key — and the
    // key is a required literal, never an index signature, which would make
    // every other read off the value a permissive hit.
    if keep_present
        && let Type::Object(object) = &peeled
        && matches!(
            property_presence_of(&peeled, property),
            PropertyPresence::Absent
        )
        && !is_object_prototype_member(property)
    {
        let mut properties = (*object.properties).clone();
        properties.insert(
            std::sync::Arc::from(property),
            surge_ts_types::ObjectProperty::required(Type::GenuineUnknown),
        );
        return Some(Type::Object(crate::metrics::alloc_object_type(
            properties,
            object.string_index_type.as_deref().cloned(),
        )));
    }
    let Type::Union(union) = &peeled else {
        return None;
    };
    let mut kept = Vec::new();
    let mut changed = false;
    for member in union.types().iter() {
        match narrow_member_by_property_presence(member, property, keep_present) {
            PresenceNarrowing::Kept => kept.push(member.clone()),
            PresenceNarrowing::Removed => changed = true,
            PresenceNarrowing::Narrowed(narrowed) => {
                changed = true;
                kept.push(narrowed);
            }
        }
    }

    if !changed || kept.is_empty() {
        return None;
    }
    Some(union_type(kept))
}

/// Parses a `"property" in object` test, returning the object expression and the
/// property name.
pub(crate) fn parse_in_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, &str)> {
    use surge_ts_syntax::ParsedBinaryOperator;
    let ParsedExpression::Binary {
        left,
        operator: ParsedBinaryOperator::In,
        right,
        ..
    } = condition
    else {
        return None;
    };
    let ParsedExpression::StringLiteral(property) = left.as_ref() else {
        return None;
    };
    Some((right.as_ref(), property.as_str()))
}

/// Builds a symbol table narrowed by a `"prop" in obj` test for the given branch.
pub(crate) fn narrow_property_presence_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (object, property) = parse_in_condition(condition)?;
    let ParsedExpression::Identifier { name, .. } = object else {
        return None;
    };
    let symbol = symbols.get(name)?;
    let narrowed = narrow_union_by_property_presence(&symbol.ty, property, branch_is_true)?;
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    narrowed_symbols.insert_narrowed(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
        symbol.ty.clone(),
    );
    Some(narrowed_symbols)
}
