use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{Type, TypeCopyReason};

use super::{const_member_literal_value, literal_expression_value, narrow_union_by_discriminant};
use crate::symbols::{SymbolInfo, SymbolTable};

/// `x?.p === lit` holds only when `x` is not nullish (a nullish `x` reads
/// `undefined`, which equals no literal), so the true branch drops `x`'s
/// `undefined` even when `p` is no discriminant — tsc's optional-chain
/// containment narrowing. `None` when nothing changes.
pub(crate) fn narrow_optional_chain_base(
    condition: &ParsedExpression,
    subject_ty: &Type,
    literal: &Type,
    keep_matching: bool,
) -> Option<Type> {
    let ParsedExpression::Binary { left, right, .. } = condition else {
        return None;
    };
    let optional_access = matches!(
        left.as_ref(),
        ParsedExpression::OptionalPropertyAccess { .. }
    ) || matches!(
        right.as_ref(),
        ParsedExpression::OptionalPropertyAccess { .. }
    );
    if !optional_access || !keep_matching || *literal == Type::Undefined {
        return None;
    }
    let narrowed = surge_ts_types::remove_nullish(subject_ty);
    (narrowed != *subject_ty && !narrowed.is_unknown()).then_some(narrowed)
}

/// A discriminant test through an optional chain narrows twice: the union by
/// the discriminant, and the base by the chain's non-nullishness. Both apply to
/// `x?.kind === "a"` on `A | B | null`, which leaves `A`.
pub(crate) fn narrow_discriminant_through_optional_chain(
    condition: &ParsedExpression,
    subject_ty: &Type,
    property: &str,
    literal: &Type,
    keep_matching: bool,
) -> Option<Type> {
    match narrow_union_by_discriminant(subject_ty, property, literal, keep_matching) {
        Some(narrowed) => Some(
            narrow_optional_chain_base(condition, &narrowed, literal, keep_matching)
                .unwrap_or(narrowed),
        ),
        None => narrow_optional_chain_base(condition, subject_ty, literal, keep_matching),
    }
}

/// `parse_discriminant_condition` with an extra resolver for operands that are
/// not literal tokens but still denote a unit literal type.
pub(crate) fn parse_discriminant_condition_with<'a>(
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
        // `x?.kind === "a"` discriminates like `x.kind === "a"`; a nullish `x`
        // reads `undefined`, which never equals a literal, so the member
        // filter drops it exactly as it drops any member without the property.
        if let ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
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
            let narrowed = narrow_discriminant_through_optional_chain(
                condition,
                &symbol.ty,
                property,
                &literal,
                keep_matching,
            )?;
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
                    readonly: base_property_type.readonly,
                    restriction: base_property_type.restriction.clone(),
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
