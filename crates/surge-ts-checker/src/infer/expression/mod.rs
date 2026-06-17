use std::time::Instant;

use std::collections::BTreeMap;

use surge_ts_syntax::ParsedExpression;
use surge_ts_types::{NumberLiteralType, ObjectType, Type, union_type};

use crate::context::CheckerContext;
use crate::infer::map_parsed_type;
use crate::program::{
    record_expression_infer, record_function_type_copy_from_expression_call_return_count,
    record_function_type_copy_from_expression_identifier_count,
    record_function_type_copy_from_expression_optional_call_return_count,
    record_object_type_clone_count, record_object_type_id_copy_count, record_program_timing,
    record_type_clone_count, record_union_type_clone_count,
    record_union_type_copy_from_expression_call_return_count,
    record_union_type_copy_from_expression_identifier_count,
    record_union_type_copy_from_expression_optional_call_return_count,
};
use crate::symbols::SymbolTable;

use super::InferredExpression;

mod access;
mod functions;
mod literals;
mod operators;

pub(crate) use access::*;
pub(crate) use functions::*;
pub(crate) use literals::*;
pub(crate) use operators::*;
enum CopySource {
    Identifier,
    CallReturn,
    OptionalCallReturn,
}

fn clone_type_with_metrics(ty: &Type, source: CopySource) -> Type {
    record_type_clone_count();
    match ty {
        Type::Object(_) => record_object_type_clone_count(),
        Type::Union(_) => record_union_type_clone_count(),
        _ => {}
    }
    if matches!(ty, Type::Object(_)) {
        record_object_type_id_copy_count();
    }
    match (source, ty) {
        (CopySource::Identifier, Type::Function(_)) => {
            record_function_type_copy_from_expression_identifier_count();
        }
        (CopySource::Identifier, Type::Union(_)) => {
            record_union_type_copy_from_expression_identifier_count();
        }
        (CopySource::CallReturn, Type::Function(_)) => {
            record_function_type_copy_from_expression_call_return_count();
        }
        (CopySource::CallReturn, Type::Union(_)) => {
            record_union_type_copy_from_expression_call_return_count();
        }
        (CopySource::OptionalCallReturn, Type::Function(_)) => {
            record_function_type_copy_from_expression_optional_call_return_count();
        }
        (CopySource::OptionalCallReturn, Type::Union(_)) => {
            record_union_type_copy_from_expression_optional_call_return_count();
        }
        _ => {}
    }

    ty.clone()
}

pub(crate) fn infer_expression(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_expression_infer();
    let infer_start = Instant::now();
    let result = match parsed_expression {
        ParsedExpression::StringLiteral(value) => {
            InferredExpression::Known(Type::StringLiteral(value.clone()))
        }
        ParsedExpression::NumberLiteral(value) => {
            InferredExpression::Known(Type::NumberLiteral(NumberLiteralType {
                value: value.clone(),
            }))
        }
        ParsedExpression::BooleanLiteral(value) => {
            InferredExpression::Known(Type::BooleanLiteral(*value))
        }
        ParsedExpression::UndefinedLiteral => InferredExpression::Known(Type::Undefined),
        ParsedExpression::NullLiteral => InferredExpression::Known(Type::Any),
        ParsedExpression::Identifier { name, span } => symbols
            .get(name)
            .map(|symbol| {
                InferredExpression::Known(clone_type_with_metrics(
                    &symbol.ty,
                    CopySource::Identifier,
                ))
            })
            .unwrap_or_else(|| InferredExpression::UnresolvedIdentifier {
                name: name.clone(),
                span: *span,
            }),
        ParsedExpression::This { .. } => symbols
            .get("this")
            .map(|symbol| {
                InferredExpression::Known(clone_type_with_metrics(
                    &symbol.ty,
                    CopySource::Identifier,
                ))
            })
            // Outside a class body `this` has no instance type here; stay
            // conservative rather than emitting an unresolved-identifier error.
            .unwrap_or(InferredExpression::Unknown),
        ParsedExpression::ObjectLiteral { properties, .. } => {
            InferredExpression::Known(infer_object_literal(properties, symbols, ctx))
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            infer_array_literal(elements, symbols, ctx)
        }
        ParsedExpression::Unary {
            operator, operand, ..
        } => infer_unary_expression(*operator, operand, symbols, ctx),
        ParsedExpression::Binary {
            operator,
            left,
            right,
            ..
        } => infer_binary_expression(*operator, left, right, symbols, ctx),
        ParsedExpression::Logical { left, right, .. } => {
            infer_logical_expression(left, right, symbols, ctx)
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => infer_conditional_expression(condition, when_true, when_false, symbols, ctx),
        ParsedExpression::PropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
        } => infer_property_access(
            object,
            object_span,
            property_name,
            property_span,
            symbols,
            ctx,
        ),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => infer_index_access(object_name, object_span, index, index_span, symbols, ctx),
        ParsedExpression::OptionalPropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
        } => infer_optional_property_access(
            object,
            object_span,
            property_name,
            property_span,
            symbols,
            ctx,
        ),
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            let left_type = infer_expression(left, symbols, ctx);
            let right_type = infer_expression(right, symbols, ctx);

            match (left_type, right_type) {
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty)) => {
                    if left_ty == Type::Any || left_ty == Type::Unknown {
                        InferredExpression::Known(left_ty)
                    } else if left_ty == Type::Undefined {
                        InferredExpression::Known(right_ty)
                    } else {
                        let filtered_left = surge_ts_types::remove_nullish(&left_ty);
                        InferredExpression::Known(union_type(vec![filtered_left, right_ty]))
                    }
                }
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::SatisfiesExpression { expression, .. } => {
            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::NonNullAssertion {
            expression,
            in_optional_chain,
            ..
        } => {
            let inferred = infer_expression(expression, symbols, ctx);
            match inferred {
                InferredExpression::Known(ty) => {
                    let filtered = surge_ts_types::remove_undefined(&ty);
                    if *in_optional_chain {
                        InferredExpression::Known(surge_ts_types::union_type(vec![
                            filtered,
                            Type::Undefined,
                        ]))
                    } else {
                        InferredExpression::Known(filtered)
                    }
                }
                InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
                    if *in_optional_chain {
                        InferredExpression::Known(Type::Undefined)
                    } else {
                        inferred
                    }
                }
                other => other,
            }
        }
        ParsedExpression::ConstAssertion { expression, .. } => {
            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::ArrowFunction(arrow_function) => InferredExpression::Known(
            Type::Function(infer_arrow_function(arrow_function.as_ref(), symbols, ctx)),
        ),
        ParsedExpression::TypeAssertion {
            expression: _, ty, ..
        } => InferredExpression::Known(map_parsed_type(ty.clone(), ctx)),
        ParsedExpression::Call {
            callee_name,
            callee_span,
            type_arguments,
            arguments,
            ..
        } => match symbols.get(callee_name) {
            Some(symbol) => match &symbol.ty {
                Type::Function(function_type) => {
                    let return_type =
                        crate::checks::call::instantiate_function_return_type_for_call(
                            function_type,
                            symbol.function_signature.as_ref(),
                            type_arguments,
                            *callee_span,
                            arguments,
                            symbols,
                            ctx,
                        );
                    InferredExpression::Known(return_type)
                }
                Type::Unknown | Type::Any => InferredExpression::Unknown,
                _ => InferredExpression::Unknown,
            },
            None => InferredExpression::Unknown,
        },
        ParsedExpression::New { callee, .. } => infer_new_expression(callee, symbols, ctx),
        ParsedExpression::PropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            ..
        } => infer_property_call(
            object,
            object_span,
            property_name,
            property_span,
            symbols,
            ctx,
        ),
        ParsedExpression::OptionalPropertyCall {
            object,
            property_name,
            ..
        } => {
            let object_type = match infer_expression(object, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                _ => return InferredExpression::Unknown,
            };

            let base_type = surge_ts_types::remove_undefined(&object_type);

            match base_type {
                Type::Any => InferredExpression::Known(Type::Any),
                _ => match base_type.get_property_access_type(property_name) {
                    Some(property_type) => {
                        let prop_base = surge_ts_types::remove_undefined(&property_type);
                        if let Type::Function(function_type) = prop_base {
                            InferredExpression::Known(union_type(vec![
                                clone_type_with_metrics(
                                    function_type.return_type(),
                                    CopySource::OptionalCallReturn,
                                ),
                                Type::Undefined,
                            ]))
                        } else {
                            InferredExpression::Unknown
                        }
                    }
                    None => InferredExpression::Unknown,
                },
            }
        }
        ParsedExpression::OptionalCall { callee, .. } => {
            let callee_type = match infer_expression(callee, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                _ => return InferredExpression::Unknown,
            };

            let base_type = surge_ts_types::remove_undefined(&callee_type);

            match base_type {
                Type::Function(function_type) => InferredExpression::Known(union_type(vec![
                    clone_type_with_metrics(function_type.return_type(), CopySource::CallReturn),
                    Type::Undefined,
                ])),
                Type::Any => InferredExpression::Known(Type::Any),
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::OptionalIndexAccess {
            object,
            object_span,
            index,
            index_span,
        } => infer_optional_index_access(object, object_span, index, index_span, symbols, ctx),
        ParsedExpression::JsxElement { .. } | ParsedExpression::JsxFragment { .. } => {
            InferredExpression::Known(jsx_element_type())
        }
        ParsedExpression::Unknown => InferredExpression::Unknown,
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.type_inference += infer_start.elapsed()
    });
    result
}

/// The conservative type assigned to every JSX element and fragment. This is a
/// parser-safe stand-in for `JSX.Element`: an opaque empty object tagged with the
/// `Element` alias name so it renders exactly as tsc does in assignability
/// diagnostics (`Type 'Element' is not assignable to type 'number'.`). It does not
/// resolve the `JSX` namespace or validate intrinsic element props — both are out
/// of scope for this slice.
pub(crate) fn jsx_element_type() -> Type {
    Type::Object(ObjectType::new(BTreeMap::new(), None).with_alias_name("Element"))
}

pub(crate) fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if *ty != Type::Unknown)
}
