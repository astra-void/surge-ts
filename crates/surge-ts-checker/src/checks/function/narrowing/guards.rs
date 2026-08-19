//! Per-guard-kind leaf narrowing: recognizing a guard condition, narrowing a
//! union at the type level, and building a narrowed symbol table for it. The
//! orchestration that applies these across scopes and composes them with truthy
//! narrowing lives in the parent module.

use surge_ts_syntax::{ParsedExpression, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type};

use crate::symbols::{SymbolInfo, SymbolTable};

/// Whether a union member's discriminant `property` is the given literal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DiscriminantMatch {
    Yes,
    No,
    Unknown,
}

fn literal_expression_value(expression: &ParsedExpression) -> Option<Type> {
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

/// A case/comparison operand written as a member of a const-enum-like object
/// (`ZodIssueCode.invalid_string`, `Codes.a`) still denotes a unit literal, so
/// it must discriminate exactly like the literal spelled inline. Reads the
/// member type out of scope instead of inferring the expression, which keeps
/// this free of diagnostics and of re-entrant inference.
pub(super) fn const_member_literal_value(
    expression: &ParsedExpression,
    symbols: &SymbolTable,
) -> Option<Type> {
    let ParsedExpression::PropertyAccess {
        object,
        property_name,
        ..
    } = expression
    else {
        return None;
    };
    let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
        return None;
    };
    let symbol_ty = symbols.get(name)?.ty.peeled();
    let Type::Object(object_type) = &symbol_ty else {
        return None;
    };
    let property = object_type.properties.get(property_name.as_str())?;
    match &property.ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
            Some(property.ty.clone())
        }
        _ => None,
    }
}

fn discriminant_match(member: &Type, property: &str, literal: &Type) -> DiscriminantMatch {
    // A discriminated-union member is often a named type (nominal reference);
    // peel it to read its discriminant property.
    let member = member.peeled();
    let Type::Object(object) = &member else {
        return DiscriminantMatch::Unknown;
    };
    let Some(property_type) = object.properties.get(property) else {
        // A member without the discriminant property cannot equal the literal.
        return DiscriminantMatch::No;
    };
    // The discriminant is often written as `typeof Codes.a`, which resolves to a
    // lazy reference around the literal rather than the literal itself.
    let property_ty = property_type.ty.peeled();
    match &property_ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
            if &property_ty == literal {
                DiscriminantMatch::Yes
            } else {
                DiscriminantMatch::No
            }
        }
        Type::Union(union) => {
            if union.types().iter().any(|ty| ty == literal) {
                // The literal is one of several possibilities; keep the member in
                // both branches conservatively.
                DiscriminantMatch::Unknown
            } else if union.types().iter().all(|ty| {
                matches!(
                    ty,
                    Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_)
                )
            }) {
                DiscriminantMatch::No
            } else {
                DiscriminantMatch::Unknown
            }
        }
        _ => DiscriminantMatch::Unknown,
    }
}

/// Narrows a discriminated union by `property` against `literal`. `keep_matching`
/// selects the members that can equal the literal (the `===` true branch); its
/// negation keeps the rest. Returns `None` when the type is not a union or the
/// condition does not partition it.
pub(super) fn narrow_union_by_discriminant(
    ty: &Type,
    property: &str,
    literal: &Type,
    keep_matching: bool,
) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| {
            let result = discriminant_match(member, property, literal);
            if keep_matching {
                result != DiscriminantMatch::No
            } else {
                result != DiscriminantMatch::Yes
            }
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// Parses an `expr === null` / `expr === undefined` (or `!==`) test. Returns the
/// tested expression and whether the operator is an equality test. `null` and
/// `undefined` are the same [`Type::Undefined`] in this model, so both spellings
/// narrow identically.
pub(super) fn parse_nullish_equality_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, bool)> {
    use surge_ts_syntax::ParsedBinaryOperator;
    let ParsedExpression::Binary {
        left,
        operator,
        right,
        ..
    } = condition
    else {
        return None;
    };
    let eq = match operator {
        ParsedBinaryOperator::StrictEquals | ParsedBinaryOperator::Equals => true,
        ParsedBinaryOperator::StrictNotEquals | ParsedBinaryOperator::NotEquals => false,
        _ => return None,
    };

    let is_nullish = |expression: &ParsedExpression| {
        matches!(
            expression,
            ParsedExpression::NullLiteral | ParsedExpression::UndefinedLiteral
        )
    };

    if is_nullish(right) && !is_nullish(left) {
        return Some((left.as_ref(), eq));
    }
    if is_nullish(left) && !is_nullish(right) {
        return Some((right.as_ref(), eq));
    }
    None
}

/// Narrows `ty` by an `=== null`/`undefined` test: the matching branch keeps only
/// the nullish member, the complement drops it. `None` when `ty` has no nullish
/// member to split on, so an unrelated type is left untouched.
pub(super) fn narrow_union_by_nullish(ty: &Type, keep_matching: bool) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    if !union
        .types()
        .iter()
        .any(|member| *member == Type::Undefined)
    {
        return None;
    }
    if keep_matching {
        return Some(Type::Undefined);
    }
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| **member != Type::Undefined)
        .cloned()
        .collect();
    if kept.is_empty() {
        return None;
    }
    Some(union_type(kept))
}

/// Symbol-table counterpart of the `ScopeStack` nullish-equality narrowing, for
/// the operands of `&&`/`||` and the arms of a conditional expression
/// (`x !== undefined && x <= y`). Handles a bare identifier or one property of
/// one.
pub(super) fn narrow_nullish_equality_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (subject, eq) = parse_nullish_equality_condition(condition)?;
    let keep_matching = branch_is_true == eq;

    match subject {
        ParsedExpression::Identifier { name, .. } => {
            let symbol = symbols.get(name)?;
            let narrowed = narrow_union_by_nullish(&symbol.ty, keep_matching)?;
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
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return None;
            };
            let symbol = symbols.get(name)?;
            let symbol_ty = symbol.ty.peeled();
            let Type::Object(object_type) = &symbol_ty else {
                return None;
            };
            let property = object_type.properties.get(property_name.as_str())?;
            // An optional property carries its `undefined` in the `optional` flag
            // rather than the type, so splitting on it also clears the flag.
            let (narrowed_ty, narrowed_optional) =
                match narrow_union_by_nullish(&property.ty, keep_matching) {
                    Some(narrowed) => (narrowed, property.optional && keep_matching),
                    None if property.optional => {
                        if keep_matching {
                            (Type::Undefined, true)
                        } else {
                            (property.ty.clone(), false)
                        }
                    }
                    None => return None,
                };

            let mut new_object = object_type.clone();
            let properties = std::sync::Arc::make_mut(&mut new_object.properties);
            properties.insert(
                property_name.as_str().into(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_ty,
                    optional: narrowed_optional,
                    method: false,
                },
            );
            let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
            narrowed_symbols.insert_narrowed(
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
                symbol.ty.clone(),
            );
            Some(narrowed_symbols)
        }
        _ => None,
    }
}

/// Parses a `base.property === literal` (or `!==`) discriminant test. Returns the
/// discriminant object expression, the property name, the literal type, and
/// whether the operator is an equality (`===`/`==`) vs inequality test.
pub(super) fn parse_discriminant_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, &str, Type, bool)> {
    parse_discriminant_condition_with(condition, &|_| None)
}

/// `parse_discriminant_condition` with an extra resolver for operands that are
/// not literal tokens but still denote a unit literal type.
pub(super) fn parse_discriminant_condition_with<'a>(
    condition: &'a ParsedExpression,
    resolve_literal: &dyn Fn(&ParsedExpression) -> Option<Type>,
) -> Option<(&'a ParsedExpression, &'a str, Type, bool)> {
    use surge_ts_syntax::ParsedBinaryOperator;
    let ParsedExpression::Binary {
        left,
        operator,
        right,
        ..
    } = condition
    else {
        return None;
    };
    let eq = match operator {
        ParsedBinaryOperator::StrictEquals | ParsedBinaryOperator::Equals => true,
        ParsedBinaryOperator::StrictNotEquals | ParsedBinaryOperator::NotEquals => false,
        _ => return None,
    };

    fn discriminant_side<'a>(
        access: &'a ParsedExpression,
        value: &ParsedExpression,
        eq: bool,
        resolve_literal: &dyn Fn(&ParsedExpression) -> Option<Type>,
    ) -> Option<(&'a ParsedExpression, &'a str, Type, bool)> {
        let literal = literal_expression_value(value).or_else(|| resolve_literal(value))?;
        if let ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } = access
        {
            return Some((object.as_ref(), property_name.as_str(), literal, eq));
        }
        None
    }

    discriminant_side(left, right, eq, resolve_literal)
        .or_else(|| discriminant_side(right, left, eq, resolve_literal))
}

/// Builds a symbol table with the discriminated union narrowed for the given
/// branch, or `None` if the condition is not a recognized discriminant test.
/// Handles a base that is a plain identifier (`x.kind === …`) or a single
/// property of one (`obj.id.kind === …` narrows `obj`'s `id` property).
pub(crate) fn narrow_discriminant_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (discriminant_object, property, literal, eq) =
        parse_discriminant_condition_with(condition, &|expression| {
            const_member_literal_value(expression, symbols)
        })?;
    let keep_matching = branch_is_true == eq;

    match discriminant_object {
        ParsedExpression::Identifier { name, .. } => {
            let symbol = symbols.get(name)?;
            let narrowed =
                narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)?;
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
        ParsedExpression::PropertyAccess {
            object,
            property_name: base_property,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return None;
            };
            let symbol = symbols.get(name)?;
            // `draft` may be typed by a named declaration (nominal reference);
            // peel it to narrow its discriminant property (`draft.identity`).
            let symbol_ty = symbol.ty.peeled();
            let Type::Object(object_type) = &symbol_ty else {
                return None;
            };
            let base_property_type = object_type.properties.get(base_property.as_str())?;
            let narrowed_property = narrow_union_by_discriminant(
                &base_property_type.ty,
                property,
                &literal,
                keep_matching,
            )?;

            let mut new_object = object_type.clone();
            let properties = std::sync::Arc::make_mut(&mut new_object.properties);
            properties.insert(
                base_property.as_str().into(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_property,
                    optional: base_property_type.optional,
                    method: base_property_type.method,
                },
            );
            let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
            narrowed_symbols.insert_narrowed(
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
                symbol.ty.clone(),
            );
            Some(narrowed_symbols)
        }
        _ => None,
    }
}

/// What a union member proves about `"prop" in value`.
enum PropertyPresence {
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

fn property_presence_of(member: &Type, property: &str) -> PropertyPresence {
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

enum PresenceNarrowing {
    Kept,
    Removed,
    Narrowed(Type),
}

/// Decides a single union member. Named aliases and interfaces arrive as nominal
/// `Type::Reference` wrappers, and an alias to a union stays a nested union after
/// peeling — both must be looked through, or every member is "undecidable" and
/// the guard narrows nothing. The unpeeled member is what the caller keeps when
/// nothing was dropped, so nominal display names survive in diagnostics.
fn narrow_member_by_property_presence(
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
pub(super) fn narrow_union_by_property_presence(
    ty: &Type,
    property: &str,
    keep_present: bool,
) -> Option<Type> {
    let peeled = ty.peeled();
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
pub(super) fn parse_in_condition(
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
pub(super) fn narrow_property_presence_symbol_table(
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

/// The `typeof` tag a type reports at runtime, or `None` for a type whose tag is
/// not statically decidable (`any`/`unknown`/`never`) — such members are kept in
/// both branches so narrowing never drops a value it cannot classify.
fn typeof_tag_of(member: &Type) -> Option<&'static str> {
    if surge_ts_types::is_global_function_interface(member) {
        return Some("function");
    }
    match member.peeled() {
        Type::Number | Type::NumberLiteral(_) => Some("number"),
        Type::String | Type::StringLiteral(_) => Some("string"),
        Type::Boolean | Type::BooleanLiteral(_) => Some("boolean"),
        Type::BigInt => Some("bigint"),
        Type::Symbol => Some("symbol"),
        Type::Undefined | Type::Void => Some("undefined"),
        Type::Function(_) => Some("function"),
        // A callable/constructible object (`typeof SomeClass`, an interface with
        // a call signature) reports `"function"` at runtime, not `"object"`.
        Type::Object(object)
            if object.call_signature().is_some() || object.construct_signature().is_some() =>
        {
            Some("function")
        }
        Type::Object(_) | Type::Array(_) | Type::Tuple(_) => Some("object"),
        _ => None,
    }
}

/// Narrows a union by a `typeof x === "tag"` guard. `keep_matching` keeps the
/// members whose runtime tag is `tag` (the `=== true` branch); otherwise removes
/// them. Members with an undecidable tag are kept either way.
pub(super) fn narrow_union_by_typeof(ty: &Type, tag: &str, keep_matching: bool) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| match typeof_tag_of(member) {
            Some(member_tag) => (member_tag == tag) == keep_matching,
            None => true,
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// Parses a `typeof x === "tag"` / `!==` guard, returning the operand expression,
/// the tag string, and whether the operator is equality (vs inequality).
pub(super) fn parse_typeof_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, &str, bool)> {
    use surge_ts_syntax::ParsedBinaryOperator;
    let ParsedExpression::Binary {
        left,
        operator,
        right,
        ..
    } = condition
    else {
        return None;
    };
    let eq = match operator {
        ParsedBinaryOperator::StrictEquals | ParsedBinaryOperator::Equals => true,
        ParsedBinaryOperator::StrictNotEquals | ParsedBinaryOperator::NotEquals => false,
        _ => return None,
    };

    fn typeof_side<'a>(
        maybe_typeof: &'a ParsedExpression,
        maybe_tag: &'a ParsedExpression,
    ) -> Option<(&'a ParsedExpression, &'a str)> {
        let ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Typeof,
            operand,
            ..
        } = maybe_typeof
        else {
            return None;
        };
        let ParsedExpression::StringLiteral(tag) = maybe_tag else {
            return None;
        };
        Some((operand.as_ref(), tag.as_str()))
    }

    let (operand, tag) = typeof_side(left, right).or_else(|| typeof_side(right, left))?;
    Some((operand, tag, eq))
}

/// Builds a symbol table narrowed by a `typeof x === "tag"` guard for the branch,
/// or `None` if the condition is not such a guard over a bare identifier union.
pub(super) fn narrow_typeof_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (operand, tag, eq) = parse_typeof_condition(condition)?;
    let ParsedExpression::Identifier { name, .. } = operand else {
        return None;
    };
    let symbol = symbols.get(name)?;
    let narrowed = narrow_union_by_typeof(&symbol.ty, tag, branch_is_true == eq)?;
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

/// Whether a union member is an instance of the class/interface named `ctor_name`
/// (a nominal name match). `Some(false)` for a member that definitely is not (a
/// primitive, or a differently-named object); `None` when undecidable (`any`/
/// `unknown`), so the member is kept in both branches.
fn instanceof_matches(member: &Type, ctor_name: &str) -> Option<bool> {
    match member {
        Type::Any | Type::Unknown | Type::GenuineUnknown => None,
        Type::String
        | Type::StringLiteral(_)
        | Type::Number
        | Type::NumberLiteral(_)
        | Type::Boolean
        | Type::BooleanLiteral(_)
        | Type::Undefined
        | Type::Void
        | Type::Never => Some(false),
        // Match nominally on the member's own name (`Blob`, `URLSearchParams`):
        // a `Type::Reference` reports its referenced name without resolving, so
        // do not peel (peeling would expand to the structural shape). Compare the
        // base name with any type arguments stripped, so a generic member
        // (`Promise<T>`, `Map<K, V>`) matches its bare constructor (`Promise`,
        // `Map`) instead of being treated as a different, undecidable type — the
        // latter left `x instanceof Promise` unable to drop the non-Promise arm.
        other => {
            let name = other.name();
            let base = name.split('<').next().unwrap_or(name.as_str());
            Some(base == ctor_name)
        }
    }
}

/// Narrows a union by an `x instanceof Ctor` guard. `keep_matching` keeps the
/// members that are instances of `Ctor` (the `=== true` branch); otherwise
/// removes them. Members whose membership is undecidable are kept either way.
pub(super) fn narrow_union_by_instanceof(
    ty: &Type,
    ctor_name: &str,
    keep_matching: bool,
) -> Option<Type> {
    // Peel a lazy/nominal reference to its structural form first: a deferred
    // generic alias such as `MaybeAsync<T>` (= `T | Promise<T>`) reaches here as a
    // `Type::Reference`, and matching `Type::Union` directly would miss the union
    // it resolves to, leaving `x instanceof Promise` unable to drop the non-Promise
    // arm.
    let peeled = ty.peeled();
    let Type::Union(union) = &peeled else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| match instanceof_matches(member, ctor_name) {
            Some(is_instance) => is_instance == keep_matching,
            None => true,
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// Parses an `x instanceof Ctor` guard, returning the operand expression and the
/// constructor identifier name. (`instanceof` has no equality polarity — the
/// then-branch always keeps the matching members.)
pub(super) fn parse_instanceof_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, &str)> {
    use surge_ts_syntax::ParsedBinaryOperator;
    let ParsedExpression::Binary {
        left,
        operator: ParsedBinaryOperator::Instanceof,
        right,
        ..
    } = condition
    else {
        return None;
    };
    let ParsedExpression::Identifier {
        name: ctor_name, ..
    } = right.as_ref()
    else {
        return None;
    };
    Some((left.as_ref(), ctor_name.as_str()))
}

/// Builds a symbol table narrowed by an `x instanceof Ctor` guard for the branch.
pub(super) fn narrow_instanceof_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (operand, ctor_name) = parse_instanceof_condition(condition)?;
    let ParsedExpression::Identifier { name, .. } = operand else {
        return None;
    };
    let symbol = symbols.get(name)?;
    let narrowed = narrow_union_by_instanceof(&symbol.ty, ctor_name, branch_is_true)?;
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

/// Parses an `Array.isArray(x)` guard, returning the argument expression.
pub(super) fn parse_array_isarray_condition(
    condition: &ParsedExpression,
) -> Option<&ParsedExpression> {
    let ParsedExpression::PropertyCall {
        object,
        property_name,
        arguments,
        ..
    } = condition
    else {
        return None;
    };
    let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
        return None;
    };
    if name != "Array" || property_name != "isArray" || arguments.len() != 1 {
        return None;
    }
    Some(&arguments[0].expression)
}

/// Narrows a union by `Array.isArray(x)`. `keep_arrays` keeps the array/tuple
/// members (the `=== true` branch); otherwise removes them. `any`/`unknown`
/// members are kept either way.
pub(super) fn narrow_union_by_arrayness(ty: &Type, keep_arrays: bool) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| match member {
            Type::Any | Type::Unknown | Type::GenuineUnknown => true,
            Type::Array(_) | Type::Tuple(_) => keep_arrays,
            // `Array<T>` / `ReadonlyArray<T>` written in generic form stays a
            // nominal reference rather than `Type::Array`, so match by name too.
            Type::Reference(reference)
                if matches!(
                    reference.id.split('\u{0}').next_back(),
                    Some("Array" | "ReadonlyArray")
                ) =>
            {
                keep_arrays
            }
            _ => !keep_arrays,
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// Builds a symbol table narrowed by an `Array.isArray(x)` guard for the branch.
pub(super) fn narrow_array_isarray_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let operand = parse_array_isarray_condition(condition)?;
    let ParsedExpression::Identifier { name, .. } = operand else {
        return None;
    };
    let symbol = symbols.get(name)?;
    let narrowed = narrow_union_by_arrayness(&symbol.ty, branch_is_true)?;
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

/// Concrete `ArrayBufferView` types: the typed arrays and `DataView`. The
/// `ArrayBuffer.isView(x)` predicate is `x is ArrayBufferView`, and a union may
/// carry either the `ArrayBufferView` interface itself or a concrete view.
const ARRAY_BUFFER_VIEW_NAMES: &[&str] = &[
    "ArrayBufferView",
    "DataView",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
];

/// Whether a union member is an `ArrayBufferView` (matched nominally on the base
/// name, ignoring any generic arguments, e.g. `ArrayBufferView<ArrayBuffer>`).
fn is_array_buffer_view_type(member: &Type) -> bool {
    let name = member.name();
    let base = name.split('<').next().unwrap_or(name.as_str());
    ARRAY_BUFFER_VIEW_NAMES.contains(&base)
}

/// Parses an `ArrayBuffer.isView(x)` guard, returning the argument expression.
pub(super) fn parse_arraybuffer_isview_condition(
    condition: &ParsedExpression,
) -> Option<&ParsedExpression> {
    let ParsedExpression::PropertyCall {
        object,
        property_name,
        arguments,
        ..
    } = condition
    else {
        return None;
    };
    let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
        return None;
    };
    if name != "ArrayBuffer" || property_name != "isView" || arguments.len() != 1 {
        return None;
    }
    Some(&arguments[0].expression)
}

/// Narrows a union by `ArrayBuffer.isView(x)`. `keep_views` keeps the
/// `ArrayBufferView` members (the `=== true` branch); otherwise removes them.
/// `any`/`unknown` members are kept either way.
pub(super) fn narrow_union_by_arraybufferview(ty: &Type, keep_views: bool) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| match member {
            Type::Any | Type::Unknown | Type::GenuineUnknown => true,
            _ => is_array_buffer_view_type(member) == keep_views,
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// A user-defined type-predicate guard extracted from a call condition
/// (`isFoo(x)` where `isFoo`'s collected signature returns `param is T`).
pub(super) struct PredicateGuardInfo {
    /// The bare-identifier argument in the tested parameter's position.
    pub(super) subject: String,
    /// The predicate's parsed target type (`T` in `param is T`), unresolved.
    pub(super) predicate_type: surge_ts_syntax::ParsedType,
    /// File the predicate signature was declared in; its module-local type
    /// names resolve under this file's scope (see
    /// [`crate::symbols::FunctionSignatureInfo::declaring_file`]).
    pub(super) declaring_file: Option<std::sync::Arc<str>>,
}

/// Extracts a user-defined type-predicate guard from a call condition. The
/// callee's collected signature must declare a non-asserts `param is T`
/// predicate over a named value parameter, the call must not be generic (an
/// unsubstituted type parameter in `T` cannot be resolved at the guard site),
/// and the argument in the tested position must be a bare identifier.
pub(super) fn parse_type_predicate_condition(
    condition: &ParsedExpression,
    signature_of: &mut dyn FnMut(
        &str,
    )
        -> Option<std::sync::Arc<crate::symbols::FunctionSignatureInfo>>,
) -> Option<PredicateGuardInfo> {
    let ParsedExpression::Call {
        callee_name,
        type_arguments,
        arguments,
        ..
    } = condition
    else {
        return None;
    };
    if !type_arguments.is_empty() {
        return None;
    }
    let signature = signature_of(callee_name)?;
    if !signature.type_parameters.is_empty() {
        return None;
    }
    let Some(surge_ts_syntax::ParsedType::Predicate(predicate)) = &signature.return_type else {
        return None;
    };
    if predicate.asserts || predicate.parameter_name == "this" {
        return None;
    }
    let predicate_type = predicate.ty.clone()?;
    let index = signature
        .parameter_names
        .iter()
        .position(|name| name.as_deref() == Some(predicate.parameter_name.as_str()))?;
    let ParsedExpression::Identifier { name: subject, .. } = &arguments.get(index)?.expression
    else {
        return None;
    };
    Some(PredicateGuardInfo {
        subject: subject.clone(),
        predicate_type,
        declaring_file: signature.declaring_file.clone(),
    })
}

/// Narrows a value tested by a `param is T` predicate. In the true branch a
/// union keeps the members assignable to `T`; when none are but `T` itself fits
/// a member (`string | undefined` guarded by `x is "a" | "b"`), the value *is*
/// `T`. The false branch removes the members assignable to `T`. A non-union
/// subject narrows to `T` in the true branch when `T` is a subtype of it.
/// Returns `None` when the guard proves nothing new (or would empty the type —
/// stay conservative rather than model `never`).
pub(super) fn narrow_by_predicate(
    ty: &Type,
    predicate: &Type,
    keep_matching: bool,
) -> Option<Type> {
    let peeled = ty.peeled();
    if let Type::Union(union) = &peeled {
        let members = union.types();
        if keep_matching {
            let matching: Vec<Type> = members
                .iter()
                .filter(|member| surge_ts_types::is_assignable_to(member, predicate))
                .cloned()
                .collect();
            if !matching.is_empty() {
                if matching.len() == members.len() {
                    return None;
                }
                return Some(union_type(matching));
            }
            if members
                .iter()
                .any(|member| surge_ts_types::is_assignable_to(predicate, member))
            {
                return Some(predicate.clone());
            }
            return None;
        }
        let remaining: Vec<Type> = members
            .iter()
            .filter(|member| !surge_ts_types::is_assignable_to(member, predicate))
            .cloned()
            .collect();
        if remaining.is_empty() || remaining.len() == members.len() {
            return None;
        }
        return Some(union_type(remaining));
    }
    if keep_matching
        && !surge_ts_types::is_assignable_to(&peeled, predicate)
        && surge_ts_types::is_assignable_to(predicate, &peeled)
    {
        return Some(predicate.clone());
    }
    None
}

/// The identifier a single type guard tests, if the guard is one we model over a
/// bare identifier (`x instanceof C`, `typeof x === "s"`, `Array.isArray(x)`,
/// `ArrayBuffer.isView(x)`).
pub(super) fn guard_operand_identifier(condition: &ParsedExpression) -> Option<&str> {
    let operand = if let Some((operand, _)) = parse_instanceof_condition(condition) {
        operand
    } else if let Some((operand, _, _)) = parse_typeof_condition(condition) {
        operand
    } else if let Some(operand) = parse_array_isarray_condition(condition) {
        operand
    } else if let Some(operand) = parse_arraybuffer_isview_condition(condition) {
        operand
    } else if let Some((operand, _)) = parse_in_condition(condition) {
        operand
    } else {
        return None;
    };
    match operand {
        ParsedExpression::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}
