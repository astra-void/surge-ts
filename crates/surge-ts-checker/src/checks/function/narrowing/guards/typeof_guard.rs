use surge_ts_syntax::{ParsedExpression, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type};

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
        Type::Object(_) | Type::Array(_) | Type::Tuple(_) => Some("object"),
        _ => None,
    }
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

pub(crate) fn narrow_union_by_typeof(ty: &Type, tag: &str, keep_matching: bool) -> Option<Type> {
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
        // The `unknown` keyword carries no tag, so it survives the filter and
        // leaves the union unassignable to anything the guard just proved. In
        // the matching branch the tag *is* the member's type, which is how tsc
        // reads `typeof x === 'string'` on an `unknown`. Only the genuine
        // keyword qualifies: `Type::Unknown` is the degradation sentinel, and
        // rewriting it would claim knowledge surge does not have.
        .map(|member| match (member, keep_matching) {
            (Type::GenuineUnknown, true) => type_for_typeof_tag(tag).unwrap_or_else(|| member.clone()),
            _ => member.clone(),
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
