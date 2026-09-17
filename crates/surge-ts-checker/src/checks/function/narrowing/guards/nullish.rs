use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{Type, TypeCopyReason, union_type};

use crate::symbols::{SymbolInfo, SymbolTable};

/// Parses an `expr === null` / `expr === undefined` (or `!==`) test. Returns the
/// tested expression and whether the operator is an equality test. `null` and
/// `undefined` are the same [`Type::Undefined`] in this model, so both spellings
/// narrow identically.
pub(crate) fn parse_nullish_equality_condition(
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
pub(crate) fn narrow_union_by_nullish(ty: &Type, keep_matching: bool) -> Option<Type> {
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
pub(crate) fn narrow_nullish_equality_symbol_table(
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
                    method: property.method,
                    readonly: property.readonly,
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
