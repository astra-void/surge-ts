//! Property access, index access, calls, and their optional-chaining variants.

use super::*;

use std::time::Instant;

use surge_ts_syntax::{ParsedExpression, TextSpan};
use surge_ts_types::{Type, is_assignable_to, union_type};

use crate::context::CheckerContext;
use crate::program::{record_program_timing, record_property_lookup};
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_index_access(
    object_name: &str,
    object_span: &Option<TextSpan>,
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name) else {
        return InferredExpression::UnresolvedIdentifier {
            name: object_name.to_string(),
            span: *object_span,
        };
    };

    match &symbol.ty {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                match ty {
                    Type::Tuple(elements) => {
                        let res =
                            infer_tuple_index_access(elements, index, index_span, symbols, ctx);
                        if let InferredExpression::Known(ty) = res {
                            result_types.push(ty);
                        } else {
                            return InferredExpression::Unknown;
                        }
                    }
                    Type::Array(element_type) => {
                        let element_type = element_type.as_ref();
                        if matches!(element_type, Type::Unknown) {
                            return InferredExpression::Unknown;
                        }

                        let index_type = match infer_expression(index, symbols, ctx) {
                            InferredExpression::Known(ty) => ty,
                            _ => return InferredExpression::Unknown,
                        };

                        if !surge_ts_types::is_assignable_to(&index_type, &Type::Number) {
                            return InferredExpression::Unknown;
                        }
                        result_types.push(element_type.clone());
                    }
                    _ => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(surge_ts_types::union_type(result_types))
        }
        Type::Tuple(elements) => {
            infer_tuple_index_access(elements, index, index_span, symbols, ctx)
        }
        Type::Array(element_type) => {
            let element_type = element_type.as_ref();
            if matches!(element_type, Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_type = match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let _ = index_span;
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(element_type.clone())
        }
        Type::Object(_)
        | Type::Function(_)
        | Type::String
        | Type::Number
        | Type::Boolean
        | Type::Void
        | Type::Never
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Undefined => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_tuple_index_access(
    elements: &[Type],
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let index_type = match infer_expression(index, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => return InferredExpression::Unknown,
    };

    if let Some(index_value) = tuple_index_value(&index_type) {
        return elements
            .get(index_value)
            .cloned()
            .map(InferredExpression::Known)
            .unwrap_or(InferredExpression::Unknown);
    }

    if is_assignable_to(&index_type, &Type::Number) {
        return InferredExpression::Known(union_type(elements.to_vec()));
    }

    let _ = index_span;
    InferredExpression::Unknown
}

pub(crate) fn infer_property_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_access_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let result = match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                match ty.get_property_access_type(property_name) {
                    Some(ty) => result_types.push(ty),
                    None => {
                        return InferredExpression::MissingProperty {
                            property_name: property_name.to_string(),
                            object_type: ty.clone(),
                            span: *property_span,
                        };
                    }
                }
            }
            InferredExpression::Known(surge_ts_types::union_type(result_types))
        }
        _ => object_type
            .get_property_access_type(property_name)
            .map(InferredExpression::Known)
            .unwrap_or_else(|| InferredExpression::MissingProperty {
                property_name: property_name.to_string(),
                object_type: object_type.clone(),
                span: *property_span,
            }),
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_access_start.elapsed()
    });
    result
}

pub(crate) fn infer_property_call(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    _property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_call_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let result = match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Array(element_type) if property_name == "find" => {
            InferredExpression::Known(surge_ts_types::union_type(vec![
                element_type.as_ref().clone(),
                Type::Undefined,
            ]))
        }
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    result_types.push(surge_ts_types::union_type(vec![
                        element_type.as_ref().clone(),
                        Type::Undefined,
                    ]));
                    continue;
                }
                match ty.get_property_access_type(property_name) {
                    Some(Type::Function(function_type)) => {
                        result_types.push(function_type.return_type().clone());
                    }
                    Some(Type::Any) => result_types.push(Type::Any),
                    Some(_) | None => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(surge_ts_types::union_type(result_types))
        }
        _ => match object_type.get_property_access_type(property_name) {
            Some(Type::Function(function_type)) => {
                InferredExpression::Known(function_type.return_type().clone())
            }
            Some(Type::Any) => InferredExpression::Known(Type::Any),
            Some(_) | None => InferredExpression::Unknown,
        },
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_call_start.elapsed()
    });
    result
}

pub(crate) fn infer_optional_index_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    index: &ParsedExpression,
    index_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = surge_ts_types::remove_undefined(&object_type);

    match &base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let result = infer_tuple_index_access(elements, index, index_span, symbols, ctx);
            match result {
                InferredExpression::Known(ty) => {
                    InferredExpression::Known(union_type(vec![ty, Type::Undefined]))
                }
                _ => result,
            }
        }
        Type::Array(element_type) => {
            let element_type = element_type.as_ref();
            if matches!(element_type, Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_type = match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Type::NumberLiteral(_) | Type::Number = index_type {
                InferredExpression::Known(union_type(vec![element_type.clone(), Type::Undefined]))
            } else {
                InferredExpression::Unknown
            }
        }
        _ => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_optional_property_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_property_lookup();
    let property_access_start = Instant::now();
    let object_type = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = surge_ts_types::remove_undefined(&object_type);

    let result_type = match base_type {
        Type::Unknown | Type::Any => InferredExpression::Known(base_type.clone()),
        Type::Union(ref union_type) => {
            let mut result_types = Vec::new();
            let mut saw_known = false;

            for ty in union_type.types() {
                if *ty == Type::Undefined || *ty == Type::Unknown {
                    continue;
                }

                saw_known = true;
                match ty.get_property_access_type(property_name) {
                    Some(property_type) => result_types.push(property_type),
                    None => {
                        return InferredExpression::MissingProperty {
                            property_name: property_name.to_string(),
                            object_type: base_type.clone(),
                            span: *property_span,
                        };
                    }
                }
            }

            if !saw_known || result_types.is_empty() {
                InferredExpression::Unknown
            } else {
                InferredExpression::Known(surge_ts_types::union_type(vec![
                    surge_ts_types::union_type(result_types),
                    Type::Undefined,
                ]))
            }
        }
        _ => base_type
            .get_property_access_type(property_name)
            .map(InferredExpression::Known)
            .unwrap_or_else(|| InferredExpression::MissingProperty {
                property_name: property_name.to_string(),
                object_type: base_type.clone(),
                span: *property_span,
            }),
    };

    let result = match result_type {
        InferredExpression::Known(ty) => {
            InferredExpression::Known(union_type(vec![ty, Type::Undefined]))
        }
        other => other,
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.property_access_checking += property_access_start.elapsed()
    });
    result
}

pub(crate) fn infer_new_expression(
    callee: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) = surge_ts_types::Type::builtin_constructor_result_type(name)
    {
        return InferredExpression::Known(result_type);
    }

    match infer_expression(callee, symbols, ctx) {
        InferredExpression::Known(Type::Function(function_type)) => {
            InferredExpression::Known(function_type.return_type().clone())
        }
        InferredExpression::Known(Type::Object(object))
            if object.construct_signature().is_some() =>
        {
            InferredExpression::Known(
                object
                    .construct_signature()
                    .expect("construct signature present")
                    .return_type()
                    .clone(),
            )
        }
        InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Any),
        InferredExpression::UnresolvedIdentifier { name, span } => {
            InferredExpression::UnresolvedIdentifier { name, span }
        }
        _ => InferredExpression::Unknown,
    }
}
