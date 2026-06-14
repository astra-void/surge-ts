//! Object/array literal inference.

use super::*;

use std::collections::BTreeMap;
use std::time::Instant;

use typescript_rust_syntax::{ParsedArrayElement, ParsedExpression, ParsedObjectProperty};
use typescript_rust_types::{ObjectProperty, Type, union_type};

use crate::arena::alloc_object_type;
use crate::context::CheckerContext;
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
    let properties = properties
        .iter()
        .map(|property| {
            record_property_lookup();
            record_object_literal_property_check();
            (
                property.name.clone(),
                ObjectProperty::required(infer_object_property_value(
                    &property.value,
                    symbols,
                    ctx,
                )),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let result = Type::Object(alloc_object_type(properties, None));
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.object_literal_checking += object_literal_start.elapsed()
    });
    result
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
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown => {
                return InferredExpression::Unknown;
            }
            InferredExpression::Known(ty) => element_types.push(ty),
        }
    }

    InferredExpression::Known(Type::Array(Box::new(union_type(element_types))))
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
