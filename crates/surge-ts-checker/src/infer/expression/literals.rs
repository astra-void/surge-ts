//! Object/array literal inference.

use super::*;

use std::time::Instant;

use surge_ts_syntax::{ParsedArrayElement, ParsedExpression, ParsedObjectProperty};
use surge_ts_types::{
    ObjectProperty, PropertyMap, Type, TypeCopyReason, union_type, with_type_copy_reason,
};

use crate::checks::function::check_arrow_function_expression;
use crate::context::CheckerContext;
use crate::metrics::alloc_object_type;
use crate::program::{
    record_object_literal_property_check, record_program_timing, record_property_lookup,
};
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_object_literal(
    properties: &[ParsedObjectProperty],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    let object_literal_start = Instant::now();
    let mut merged_properties: PropertyMap = PropertyMap::default();
    // A spread source that surge could not fully enumerate carries
    // `synthetic_open_index`. The members it stands for are real — the derived
    // interface was kept open precisely because they exist — so the spread
    // result must stay open too; a closed result reports every one of them as a
    // missing property (tanstack's `defaultQueryOptions` spreads
    // `QueryObserverOptions`, whose `extends WithRequired<QueryOptions<…>,
    // 'queryKey'>` base degrades, and read `queryHash`/`networkMode`/`persister`
    // /`queryFn` off the result). A *declared* index signature is deliberately
    // not propagated here; only surge's own openness marker is.
    let mut spread_source_is_open = false;
    let mut spread_source_is_any = false;
    for property in properties {
        record_property_lookup();
        record_object_literal_property_check();

        if property.is_spread {
            // `{ ...source }` merges `source`'s own properties; later properties
            // (including later spreads) override earlier ones, matching tsc's
            // left-to-right spread semantics. The source is peeled so a nominal
            // reference (`const d: Props = …; { ...d }`) contributes its members.
            // A spread whose type we cannot model as an object is skipped rather
            // than collapsing the whole literal.
            match infer_object_property_value(&property.value, symbols, ctx).peeled() {
                // `{ ...anyValue, k: v }` is `any` in tsc: the spread can carry
                // anything, so the literal has no knowable shape at all. The
                // remaining properties are still walked for their own
                // diagnostics; only the resulting type collapses.
                Type::Any => {
                    spread_source_is_any = true;
                }
                // Surge's own degradation sentinel: the members it stands for
                // are real but unenumerable, so the result must stay open.
                Type::Unknown => {
                    spread_source_is_open = true;
                }
                Type::Object(source) => {
                    spread_source_is_open |= source.synthetic_open_index;
                    for (name, source_property) in source.properties.iter() {
                        merged_properties.insert(name.clone(), source_property.clone());
                    }
                }
                // `{ ...(cond ? { list } : {}) }`: spreading a union contributes
                // each member's properties, and a property absent from (or
                // optional in) some member becomes optional — the shape tsc
                // infers for a conditional spread.
                Type::Union(source) => {
                    spread_source_is_open |= source.types().iter().any(|member| {
                        matches!(member, Type::Object(object) if object.synthetic_open_index)
                    });
                    merge_union_spread(&source, &mut merged_properties);
                }
                _ => continue,
            }
            continue;
        }

        merged_properties.insert(
            property.name.as_str().into(),
            ObjectProperty::required(infer_object_property_type(property, symbols, ctx)),
        );
    }

    let result = if spread_source_is_any {
        Type::Any
    } else if spread_source_is_open {
        let mut object = alloc_object_type(merged_properties, Some(Type::Any));
        object = object.with_open_index_marker();
        Type::Object(object)
    } else {
        Type::Object(alloc_object_type(merged_properties, None))
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.object_literal_checking += object_literal_start.elapsed()
    });
    result
}

fn merge_union_spread(source: &surge_ts_types::UnionType, merged: &mut PropertyMap) {
    let members: Vec<Option<&surge_ts_types::ObjectType>> = source
        .types()
        .iter()
        .map(|member| match member {
            Type::Object(object) => Some(object),
            // `undefined`/`null` (and anything else with no own properties)
            // contributes nothing but still makes the other members' properties
            // optional.
            _ => None,
        })
        .collect();

    if members.iter().all(Option::is_none) {
        return;
    }

    let mut names: Vec<std::sync::Arc<str>> = Vec::new();
    for member in members.iter().flatten() {
        for (name, _) in member.properties.iter() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }

    for name in names {
        let mut present_types = Vec::new();
        let mut missing_or_optional = false;
        for member in &members {
            match member.and_then(|object| object.properties.get(&name)) {
                Some(property) => {
                    present_types.push(property.ty.clone());
                    missing_or_optional |= property.is_optional();
                }
                None => missing_or_optional = true,
            }
        }

        // An earlier required property is not weakened by a later conditional
        // spread: `{ a: 1, ...(cond ? { a: 2 } : {}) }` still always has `a`.
        let already_required = merged.get(&name).is_some_and(ObjectProperty::is_required);
        let ty = union_type(present_types);
        merged.insert(
            name,
            if missing_or_optional && !already_required {
                ObjectProperty::optional(ty)
            } else {
                ObjectProperty::required(ty)
            },
        );
    }
}

/// A `[…] as const` argument, typed the way the assertion says: an array literal
/// is a tuple and a nested one is a nested tuple, all the way down. The inference
/// sketch previously forwarded straight through the assertion, so
/// `flatMap(range, (x) => [[x, x, x]] as const)` handed `number[][]` to the
/// inference of `readonly B[]` and `B` came out as `number[]` instead of the inner
/// tuple. Leaves go through ordinary inference, so their literal types survive.
pub(crate) fn infer_const_expression(
    expression: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let mut element_types = Vec::with_capacity(elements.len());
            for element in elements {
                match infer_const_expression(&element.expression, symbols, ctx) {
                    InferredExpression::Known(ty) if !ty.is_unknown() => element_types.push(ty),
                    _ => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(Type::Tuple(element_types))
        }
        ParsedExpression::ObjectLiteral { properties, .. }
            if !properties.iter().any(|property| property.is_spread) =>
        {
            let mut members = surge_ts_types::PropertyMap::default();
            for property in properties {
                match infer_const_expression(&property.value, symbols, ctx) {
                    InferredExpression::Known(ty) if !ty.is_unknown() => {
                        members.insert(
                            property.name.as_str().into(),
                            surge_ts_types::ObjectProperty::required(ty),
                        );
                    }
                    _ => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(Type::Object(crate::metrics::alloc_object_type(
                members, None,
            )))
        }
        _ => infer_expression(expression, symbols, ctx),
    }
}

pub(crate) fn infer_array_literal(
    elements: &[ParsedArrayElement],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    if elements.is_empty() {
        return InferredExpression::Known(Type::Array(Box::new(Type::Any)));
    }

    let mut element_types = Vec::new();

    for element in elements {
        match infer_expression(&element.expression, symbols, ctx) {
            InferredExpression::Known(Type::Any) => {
                return InferredExpression::Known(Type::Array(Box::new(Type::Any)));
            }
            InferredExpression::Known(Type::Unknown)
            | InferredExpression::Known(Type::GenuineUnknown)
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
            InferredExpression::Known(ty) => element_types.push(ty),
        }
    }

    // A bare array literal widens its element literals like tsc does
    // (`["a", "b"]` -> `string[]`, not `("a" | "b")[]`), so methods such as
    // `["a","b"].includes(someString)` accept a widened argument. Contextual
    // typing against a literal-union target goes through a different path and is
    // unaffected.
    let element_type = crate::checks::expr::widen_type(&union_type(element_types));
    InferredExpression::Known(Type::Array(Box::new(element_type)))
}

pub(crate) fn infer_object_property_value(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    match infer_expression(parsed_expression, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        _ => Type::Unknown,
    }
}

/// Infers the type of an object literal property. Method shorthand is lowered to an arrow
/// function whose declared parameter and return types must be honored, so it is routed through
/// the arrow-function checking path (which also checks the body, consistent with function
/// declarations) rather than the inference path that widens inline parameters to `any`.
fn infer_object_property_type(
    property: &ParsedObjectProperty,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    if (property.is_method || property.is_accessor)
        && let ParsedExpression::ArrowFunction(arrow) = &property.value
    {
        let function_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
            check_arrow_function_expression(arrow.as_ref().clone(), symbols, ctx)
        });
        if property.is_accessor {
            // A getter takes no parameters and yields its return type; a setter
            // takes one and yields that parameter's type.
            return match function_type.parameters().first() {
                Some(parameter) => parameter.clone(),
                None => function_type.return_type().clone(),
            };
        }
        return Type::Function(function_type);
    }

    infer_object_property_value(&property.value, symbols, ctx)
}
