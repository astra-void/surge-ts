use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{Type, TypeCopyReason, union_type};

use crate::symbols::{SymbolInfo, SymbolTable};

/// Which nullish values an equality test against `null`/`undefined` selects.
/// A strict test names one value; a loose `==` against either names both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NullishTest {
    pub(crate) null: bool,
    pub(crate) undefined: bool,
}

impl NullishTest {
    fn selects(self, member: &Type) -> bool {
        match member {
            Type::Null => self.null,
            Type::Undefined => self.undefined,
            // `T & null` (a type variable narrowed by `typeof x === "object"`)
            // has the facts of its nullish operand.
            Type::Object(object) if surge_ts_types::type_variable::is_nullish_type_variable_intersection(member) => {
                object.intersection_operands.as_deref().is_some_and(|operands| {
                    operands.iter().any(|operand| match operand {
                        Type::Null => self.null,
                        Type::Undefined => self.undefined,
                        _ => false,
                    })
                })
            }
            _ => false,
        }
    }
}

/// Parses an `expr === null` / `expr === undefined` (or `!==`, `==`, `!=`)
/// test. Returns the tested expression, whether the operator is an equality
/// test, and which nullish values it selects.
pub(crate) fn parse_nullish_equality_condition(
    condition: &ParsedExpression,
) -> Option<(&ParsedExpression, bool, NullishTest)> {
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
    let (eq, strict) = match operator {
        ParsedBinaryOperator::StrictEquals => (true, true),
        ParsedBinaryOperator::Equals => (true, false),
        ParsedBinaryOperator::StrictNotEquals => (false, true),
        ParsedBinaryOperator::NotEquals => (false, false),
        _ => return None,
    };

    let is_nullish = |expression: &ParsedExpression| {
        matches!(
            expression,
            ParsedExpression::NullLiteral | ParsedExpression::UndefinedLiteral
        )
    };
    let test = |literal: &ParsedExpression| {
        let is_null = matches!(literal, ParsedExpression::NullLiteral);
        NullishTest {
            null: is_null || !strict,
            undefined: !is_null || !strict,
        }
    };

    if is_nullish(right) && !is_nullish(left) {
        return Some((left.as_ref(), eq, test(right)));
    }
    if is_nullish(left) && !is_nullish(right) {
        return Some((right.as_ref(), eq, test(left)));
    }
    None
}

/// Narrows `ty` by a nullish equality test: the matching branch keeps only the
/// selected nullish members, the complement drops them. `None` when `ty` has no
/// selected member to split on, so an unrelated type is left untouched.
pub(crate) fn narrow_union_by_nullish(
    ty: &Type,
    keep_matching: bool,
    test: NullishTest,
) -> Option<Type> {
    let Type::Union(union) = ty else {
        // A binding narrowed to exactly the tested value is `never` where the
        // test fails: `m === null || v > m` reads `m` as `never` on the right
        // when `m` was `null`.
        return (!keep_matching && test.selects(ty)).then_some(Type::Never);
    };
    if !union.types().iter().any(|member| test.selects(member)) {
        return None;
    }
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| test.selects(member) == keep_matching)
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
pub(crate) fn narrow_nullish_equality_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (subject, eq, test) = parse_nullish_equality_condition(condition)?;
    let keep_matching = branch_is_true == eq;

    match subject {
        ParsedExpression::Identifier { name, .. } => {
            let symbol = symbols.get(name)?;
            let narrowed = narrow_union_by_nullish(&symbol.ty, keep_matching, test)?;
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
                match narrow_union_by_nullish(&property.ty, keep_matching, test) {
                    Some(narrowed) => {
                        (narrowed, property.optional && keep_matching == test.undefined)
                    }
                    None if property.optional && test.undefined => {
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
                    method: property.method,
                    readonly: property.readonly,
                    restriction: property.restriction.clone(),
                    index_slot: property.index_slot,
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
