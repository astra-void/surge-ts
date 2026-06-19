//! Truthiness-guard narrowing of identifiers and properties within function bodies.

use std::sync::Arc;
use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::checks::expr::evaluate_expression;
use crate::checks::ops;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TruthyGuardTarget {
    Identifier(String),
    Property { base: String, property: String },
}

pub(crate) fn narrow_truthy_guarded_identifiers(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
) {
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = scopes.resolve(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    surge_ts_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        let _ = scopes.update_visible(
            base_name,
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }
}

pub(crate) fn collect_truthy_guarded_identifiers(
    condition: &ParsedExpression,
    targets: &mut Vec<TruthyGuardTarget>,
) {
    match condition {
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            collect_truthy_guarded_identifiers(left, targets);
            collect_truthy_guarded_identifiers(right, targets);
        }
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => {
            if let Some(target) = truthy_guard_target(operand) {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn truthy_guard_target(expression: &ParsedExpression) -> Option<TruthyGuardTarget> {
    match expression {
        ParsedExpression::Identifier { name, .. } => {
            Some(TruthyGuardTarget::Identifier(name.clone()))
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => truthy_guard_base_identifier(object).map(|base| TruthyGuardTarget::Property {
            base,
            property: property_name.clone(),
        }),
        ParsedExpression::NonNullAssertion { expression, .. } => truthy_guard_target(expression),
        _ => None,
    }
}

pub(crate) fn truthy_guard_base_identifier(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::NonNullAssertion { expression, .. } => {
            truthy_guard_base_identifier(expression)
        }
        _ => None,
    }
}

pub(crate) fn narrow_truthy_guarded_property(ty: &Type, property: &str) -> Type {
    let narrowed_base = surge_ts_types::remove_undefined(&ty.peeled());

    match narrowed_base {
        Type::Object(mut object_type) => {
            if let Some(existing) = object_type.properties.get(property).cloned() {
                let properties = Arc::make_mut(&mut object_type.properties);
                properties.insert(
                    property.to_string(),
                    surge_ts_types::ObjectProperty {
                        ty: surge_ts_types::remove_undefined(&existing.ty),
                        optional: false,
                    },
                );
            }

            Type::Object(object_type)
        }
        Type::Union(union) => union_type(
            union
                .types()
                .iter()
                .map(|member| narrow_truthy_guarded_property(member, property))
                .collect(),
        ),
        _ => narrowed_base,
    }
}

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
fn narrow_union_by_discriminant(
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
fn parse_discriminant_condition(
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
            let narrowed = narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)?;
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
            let base_property_type = object_type.properties.get(base_property)?;
            let narrowed_property =
                narrow_union_by_discriminant(&base_property_type.ty, property, &literal, keep_matching)?;

            let mut new_object = object_type.clone();
            let properties = std::sync::Arc::make_mut(&mut new_object.properties);
            properties.insert(
                base_property.clone(),
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
fn narrow_property_presence_symbol_table(
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

/// Narrows for a branch by any recognized type guard: a discriminated-union
/// equality test (`x.kind === "a"`) or an `in` property-presence test
/// (`"prop" in x`). Returns `None` if the condition is neither.
pub(crate) fn narrow_condition_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    narrow_discriminant_symbol_table(condition, symbols, branch_is_true)
        .or_else(|| narrow_property_presence_symbol_table(condition, symbols, branch_is_true))
}

/// Like [`narrow_discriminant_symbol_table`] but applies the narrowing in place
/// to a `ScopeStack`, for narrowing a discriminated union inside an `if` branch
/// (or after an early-returning `if`).
pub(crate) fn narrow_discriminant_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) {
    let Some((discriminant_object, property, literal, eq)) =
        parse_discriminant_condition(condition)
    else {
        return;
    };
    let keep_matching = branch_is_true == eq;

    let (base_name, narrowed_symbol) = match discriminant_object {
        ParsedExpression::Identifier { name, .. } => {
            let Some(symbol) = scopes.resolve(name) else {
                return;
            };
            let Some(narrowed) =
                narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)
            else {
                return;
            };
            (
                name.clone(),
                SymbolInfo {
                    ty: narrowed,
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
            )
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name: base_property,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return;
            };
            let Some(symbol) = scopes.resolve(name) else {
                return;
            };
            // Peel a named-typed base (`draft: Draft`) to narrow its discriminant
            // property in scope.
            let symbol_ty = symbol.ty.peeled();
            let Type::Object(object_type) = &symbol_ty else {
                return;
            };
            let Some(base_property_type) = object_type.properties.get(base_property) else {
                return;
            };
            let Some(narrowed_property) = narrow_union_by_discriminant(
                &base_property_type.ty,
                property,
                &literal,
                keep_matching,
            ) else {
                return;
            };
            let mut new_object = object_type.clone();
            let properties = std::sync::Arc::make_mut(&mut new_object.properties);
            properties.insert(
                base_property.clone(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_property,
                    optional: base_property_type.optional,
                },
            );
            (
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
            )
        }
        _ => return,
    };

    let _ = scopes.update_visible(&base_name, narrowed_symbol);
}

pub(crate) fn evaluate_condition_expression_with_truthy_guards(
    expression: &ParsedExpression,
    fallback_span: Option<surge_ts_syntax::TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::Logical {
            left,
            left_span,
            operator: ParsedLogicalOperator::Or,
            right,
            right_span,
            ..
        } => {
            let left_result = evaluate_condition_expression_with_truthy_guards(
                left,
                left_span.or(fallback_span),
                symbols,
                ctx,
            );
            let narrowed_symbols = narrow_truthy_guarded_symbol_table(left, symbols);
            let right_result = evaluate_condition_expression_with_truthy_guards(
                right,
                right_span.or(fallback_span),
                &narrowed_symbols,
                ctx,
            );
            ops::evaluate_logical_expression(
                ParsedLogicalOperator::Or,
                left_result,
                right_result,
            )
        }
        _ => evaluate_expression(expression, fallback_span, symbols, ctx),
    }
}

pub(crate) fn narrow_truthy_guarded_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
) -> SymbolTable {
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = narrowed_symbols.get(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    surge_ts_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        narrowed_symbols.insert(
            base_name.clone(),
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }

    narrowed_symbols
}
