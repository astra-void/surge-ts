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
    match &property_type.ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => {
            if &property_type.ty == literal {
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

/// Parses a `base.property === literal` (or `!==`) discriminant test. Returns the
/// discriminant object expression, the property name, the literal type, and
/// whether the operator is an equality (`===`/`==`) vs inequality test.
pub(super) fn parse_discriminant_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, &str, Type, bool)> {
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
    ) -> Option<(&'a ParsedExpression, &'a str, Type, bool)> {
        let literal = literal_expression_value(value)?;
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

    discriminant_side(left, right, eq).or_else(|| discriminant_side(right, left, eq))
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
    let (discriminant_object, property, literal, eq) = parse_discriminant_condition(condition)?;
    let keep_matching = branch_is_true == eq;

    match discriminant_object {
        ParsedExpression::Identifier { name, .. } => {
            let symbol = symbols.get(name)?;
            let narrowed =
                narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)?;
            let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
            narrowed_symbols.insert(
                name.clone(),
                SymbolInfo {
                    ty: narrowed,
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
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
                },
            );
            let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
            narrowed_symbols.insert(
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
            );
            Some(narrowed_symbols)
        }
        _ => None,
    }
}

/// Narrows a union by whether each member has `property` (`"prop" in obj`).
/// `keep_present` selects members that have it (the `in` true branch).
fn narrow_union_by_property_presence(
    ty: &Type,
    property: &str,
    keep_present: bool,
) -> Option<Type> {
    let Type::Union(union) = ty else {
        return None;
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| match member {
            Type::Object(object) => object.properties.contains_key(property) == keep_present,
            // Non-object members: cannot decide presence; keep conservatively.
            _ => true,
        })
        .cloned()
        .collect();

    if kept.is_empty() || kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// Parses a `"property" in object` test, returning the object expression and the
/// property name.
fn parse_in_condition(condition: &ParsedExpression) -> Option<(&ParsedExpression, &str)> {
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
    narrowed_symbols.insert(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
    );
    Some(narrowed_symbols)
}

/// The `typeof` tag a type reports at runtime, or `None` for a type whose tag is
/// not statically decidable (`any`/`unknown`/`never`) — such members are kept in
/// both branches so narrowing never drops a value it cannot classify.
fn typeof_tag_of(member: &Type) -> Option<&'static str> {
    match member.peeled() {
        Type::Number | Type::NumberLiteral(_) => Some("number"),
        Type::String | Type::StringLiteral(_) => Some("string"),
        Type::Boolean | Type::BooleanLiteral(_) => Some("boolean"),
        Type::BigInt => Some("bigint"),
        Type::Symbol => Some("symbol"),
        Type::Undefined | Type::Void => Some("undefined"),
        Type::Function(_) => Some("function"),
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
    narrowed_symbols.insert(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
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
    narrowed_symbols.insert(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
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
    narrowed_symbols.insert(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
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
    } else {
        return None;
    };
    match operand {
        ParsedExpression::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}
