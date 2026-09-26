use surge_ts_syntax::{ParsedExpression, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to, union_type};

use crate::symbols::{SymbolInfo, SymbolTable};

/// The `typeof` tag a type reports at runtime, or `None` for a type whose tag is
/// not statically decidable (`any`/`unknown`/`never`) — such members are kept in
/// both branches so narrowing never drops a value it cannot classify.
pub(super) fn typeof_tag_of(member: &Type) -> Option<&'static str> {
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
        Type::Object(_) | Type::Array(_) | Type::Tuple(_) | Type::Null => Some("object"),
        _ => None,
    }
}

/// Every `typeof` tag a value of `ty` can report, or `None` when some member's
/// tag cannot be decided (`any`, `unknown`, an unmodelled shape).
pub(crate) fn typeof_tags_of(ty: &Type) -> Option<Vec<&'static str>> {
    let members: Vec<Type> = match ty.peeled() {
        Type::Union(union) => union.types().to_vec(),
        other => vec![other],
    };
    let mut tags = Vec::new();
    for member in &members {
        let tag = match member {
            Type::Boolean | Type::BooleanLiteral(_) => "boolean",
            other => typeof_tag_of(other)?,
        };
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    Some(tags)
}

/// Narrows a union by a `typeof x === "tag"` guard. `keep_matching` keeps the
/// members whose runtime tag is `tag` (the `=== true` branch); otherwise removes
/// them. Members with an undecidable tag are kept either way.
/// The type a `typeof x === "<tag>"` test proves for a subject that carries no
/// tag of its own. Only the tags that name exactly one type qualify: `"object"`
/// admits every object shape plus `null`, and `"function"` every signature.
pub(super) fn type_for_typeof_tag(tag: &str) -> Option<Type> {
    match tag {
        "string" => Some(Type::String),
        "number" => Some(Type::Number),
        "boolean" => Some(Type::Boolean),
        "bigint" => Some(Type::BigInt),
        "symbol" => Some(Type::Symbol),
        "undefined" => Some(Type::Undefined),
        _ => None,
    }
}

/// tsc's `getNarrowedType` for a receiver that is not a union: the tag's own
/// type stands in when it is a subtype of what the receiver holds
/// (`let a: {}` tested `typeof a === "number"` is a `number`), the receiver
/// stands when it is the narrower of the two (`{ q: number }` tested for
/// `"object"`), and a tag the receiver can never report leaves the branch
/// unreachable.
fn narrow_non_union_by_typeof(ty: &Type, tag: &str, keep_matching: bool) -> Option<Type> {
    // A lone primitive the test rules out leaves the branch unreachable
    // (`typeof x === "number"` after `x` narrowed to `string`), which is how an
    // exhaustive chain of `typeof` checks reaches `never`. Object and function
    // tags are left alone: surge's object shapes do not always carry the call
    // signature that decides between them.
    let ruled_out = || {
        typeof_tag_of(ty)
            .filter(|member_tag| !matches!(*member_tag, "object" | "function"))
            .filter(|member_tag| (*member_tag == tag) != keep_matching)
            .map(|_| Type::Never)
    };
    if !keep_matching {
        // A variable the true branch keeps whole (its constraint *is* the
        // tag's type) has nothing left for the false branch.
        if let Some(narrowed) = narrow_type_variable_by_typeof(ty, tag) {
            return (narrowed == *ty).then_some(Type::Never);
        }
        return ruled_out();
    }
    if let Some(narrowed) = narrow_type_variable_by_typeof(ty, tag) {
        return Some(narrowed);
    }
    // `narrowTypeByTypeName`: a primitive tag's type is a subtype of `any` and
    // replaces it; `"object"` and `"function"` leave `any` whole.
    if matches!(ty, Type::Any) {
        return type_for_typeof_tag(tag);
    }
    if matches!(ty, Type::Unknown | Type::ErrorType | Type::TypeParameter(_)) {
        return None;
    }
    let Some(candidate) = type_for_typeof_tag(tag) else {
        return ruled_out();
    };
    if matches!(ty, Type::GenuineUnknown) {
        return Some(candidate);
    }
    if is_assignable_to(&candidate, ty) {
        return (candidate != *ty).then_some(candidate);
    }
    if is_assignable_to(ty, &candidate) {
        return None;
    }
    ruled_out().or(Some(Type::Never))
}

/// A tag no value reports (`typeof x === "Object"`, already TS2367): tsc still
/// narrows by it, and the two branches split the subject by primitiveness —
/// nothing primitive can be hiding behind an unrecognized tag, and nothing
/// else can be ruled out by one.
fn narrow_by_unmatched_typeof_tag(ty: &Type, keep_matching: bool) -> Option<Type> {
    let members: Vec<Type> = match ty {
        Type::Union(union) => union.types().to_vec(),
        other => vec![other.clone()],
    };
    let primitive = |member: &Type| {
        matches!(
            typeof_tag_of(member),
            Some("string" | "number" | "boolean" | "bigint" | "symbol" | "undefined")
        )
    };
    let kept: Vec<Type> = members
        .iter()
        .filter(|member| primitive(member) != keep_matching)
        .cloned()
        .collect();
    if kept.is_empty() {
        return Some(Type::Never);
    }
    if kept.len() == members.len() {
        return None;
    }
    Some(union_type(kept))
}

pub(crate) fn narrow_union_by_typeof(ty: &Type, tag: &str, keep_matching: bool) -> Option<Type> {
    if let Some(flattened) = surge_ts_types::flatten_reference_unions(ty) {
        return narrow_union_by_typeof(&flattened, tag, keep_matching);
    }
    if type_for_typeof_tag(tag).is_none() && !matches!(tag, "object" | "function") {
        return narrow_by_unmatched_typeof_tag(ty, keep_matching);
    }
    let Type::Union(union) = ty else {
        return narrow_non_union_by_typeof(ty, tag, keep_matching);
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter_map(|member| match narrow_type_variable_by_typeof(member, tag) {
            Some(narrowed) if keep_matching => (narrowed != Type::Never).then_some(narrowed),
            Some(narrowed) if narrowed == *member => None,
            _ => Some(member.clone()),
        })
        .filter(|member| match typeof_tag_of(member) {
            Some(member_tag) => (member_tag == tag) == keep_matching,
            None => true,
        })
        // The `unknown` keyword carries no tag, so it survives the filter and
        // leaves the union unassignable to anything the guard just proved. In
        // the matching branch the tag *is* the member's type, which is how tsc
        // reads `typeof x === 'string'` on an `unknown`. Only the genuine
        // keyword qualifies: `Type::Unknown` is the degradation sentinel, and
        // rewriting it would claim knowledge surge does not have.
        .map(|member| match (&member, keep_matching) {
            (Type::GenuineUnknown, true) => type_for_typeof_tag(tag).unwrap_or(member),
            _ => member,
        })
        .collect();

    // Nothing survived: every member carried a tag (an untagged one is kept by
    // the filter above) and none of them lands in this branch, so the branch is
    // unreachable and tsc types the subject `never`. Answering "no narrowing"
    // instead left the whole union standing, which is how
    // `typeof connection === 'string' ? createClient({ url: connection }) : …`
    // over a `Config | undefined` reported the union against `string` — tsc
    // checks `never` there and says nothing.
    if kept.is_empty() {
        return Some(Type::Never);
    }
    if kept.len() == union.types().len() {
        return None;
    }
    Some(union_type(kept))
}

/// A guard compared against a boolean literal (`isObject(o) === false`,
/// `guard(x) !== true`) is the same guard with a possibly flipped branch.
/// Returns the inner condition and whether the polarity flips.
///
/// Restricted to call-shaped and negated inner expressions: a bare
/// `flag === false` is an ordinary equality test, not a type guard, and
/// rewriting it would change unrelated truthiness narrowing.
pub(crate) fn strip_boolean_literal_comparison(
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
    let equality = match operator {
        ParsedBinaryOperator::StrictEquals | ParsedBinaryOperator::Equals => true,
        ParsedBinaryOperator::StrictNotEquals | ParsedBinaryOperator::NotEquals => false,
        _ => return None,
    };
    fn guard_shaped(expression: &ParsedExpression) -> bool {
        matches!(
            expression,
            ParsedExpression::Call { .. }
                | ParsedExpression::PropertyCall { .. }
                | ParsedExpression::Unary {
                    operator: ParsedUnaryOperator::Not,
                    ..
                }
        )
    }
    let (inner, literal) = match (left.as_ref(), right.as_ref()) {
        (inner, ParsedExpression::BooleanLiteral(literal)) if guard_shaped(inner) => {
            (inner, *literal)
        }
        (ParsedExpression::BooleanLiteral(literal), inner) if guard_shaped(inner) => {
            (inner, *literal)
        }
        _ => return None,
    };
    Some((inner, equality != literal))
}

/// Parses a `typeof x === "tag"` / `!==` guard, returning the operand expression,
/// the tag string, and whether the operator is equality (vs inequality).
pub(crate) fn parse_typeof_condition(
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
pub(crate) fn narrow_typeof_symbol_table(
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

/// tsc's `narrowTypeByTypeFacts` for a type variable of the body being checked
/// in the matching branch: it stays itself when its constraint is already the
/// tag's type, becomes `never` when its constraint can never report the tag,
/// and otherwise is intersected with it (`T & string`). `None` for anything
/// that is not such a variable, and for the `"object"`/`"function"` tags, whose
/// implied types surge does not model as operands.
fn narrow_type_variable_by_typeof(member: &Type, tag: &str) -> Option<Type> {
    let Type::TypeParameter(parameter) = member else {
        return None;
    };
    let constraint = surge_ts_types::type_variable::active_constraint(parameter)?;
    if tag == "object" {
        return narrow_type_variable_to_object(member, constraint);
    }
    let implied = type_for_typeof_tag(tag)?;
    let constraint = constraint.unwrap_or(Type::GenuineUnknown);
    if !constraint.is_unknown() && is_assignable_to(&constraint, &implied) {
        return Some(member.clone());
    }
    if narrow_union_by_typeof(&constraint, tag, true) == Some(Type::Never) {
        return Some(Type::Never);
    }
    Some(surge_ts_types::type_variable::intersect_type_variable(member, implied))
}

/// `narrowTypeByTypeName("object")`: the non-primitive part of the variable
/// and, under `strictNullChecks`, its `null` part — `(T & object) | (T & null)`,
/// whose second member a later truthiness test removes. A constraint that is
/// already an object keeps the variable whole.
fn narrow_type_variable_to_object(member: &Type, constraint: Option<Type>) -> Option<Type> {
    let object = Type::Object(
        surge_ts_types::ObjectType::new(Default::default(), None).with_non_primitive_marker(),
    );
    if let Some(constraint) = constraint.as_ref().filter(|constraint| !constraint.is_unknown()) {
        if is_assignable_to(constraint, &object) {
            return Some(member.clone());
        }
        if narrow_union_by_typeof(constraint, "object", true) == Some(Type::Never) {
            return Some(Type::Never);
        }
    }
    let object_part = surge_ts_types::type_variable::intersect_type_variable(member, object);
    if !surge_ts_types::strict_null_checks() {
        return Some(object_part);
    }
    Some(union_type(vec![
        object_part,
        surge_ts_types::type_variable::intersect_type_variable(member, Type::Null),
    ]))
}
