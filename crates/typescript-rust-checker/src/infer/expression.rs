use std::collections::BTreeMap;
use std::time::Instant;

use typescript_rust_syntax::{
    ParsedArrayElement, ParsedArrowFunction, ParsedArrowFunctionBody, ParsedBinaryOperator,
    ParsedExpression, ParsedObjectProperty, ParsedUnaryOperator, TextSpan,
};
use typescript_rust_types::{
    NumberLiteralType, ObjectProperty, Type, is_assignable_to, union_type,
};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::CheckerContext;
use crate::infer::map_parsed_type;
use crate::program::{
    record_expression_infer, record_function_type_copy_from_expression_call_return_count,
    record_function_type_copy_from_expression_identifier_count,
    record_function_type_copy_from_expression_optional_call_return_count,
    record_object_literal_property_check, record_object_type_clone_count,
    record_object_type_id_copy_count, record_program_timing, record_property_lookup,
    record_type_clone_count, record_union_type_clone_count,
    record_union_type_copy_from_expression_call_return_count,
    record_union_type_copy_from_expression_identifier_count,
    record_union_type_copy_from_expression_optional_call_return_count,
};
use crate::symbols::SymbolTable;

use super::InferredExpression;

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
                        let filtered_left = typescript_rust_types::remove_nullish(&left_ty);
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
                    let filtered = typescript_rust_types::remove_undefined(&ty);
                    if *in_optional_chain {
                        InferredExpression::Known(typescript_rust_types::union_type(vec![
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
            callee_span: _,
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

            let base_type = typescript_rust_types::remove_undefined(&object_type);

            match base_type {
                Type::Any => InferredExpression::Known(Type::Any),
                _ => match base_type.get_property_access_type(property_name) {
                    Some(property_type) => {
                        let prop_base = typescript_rust_types::remove_undefined(&property_type);
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

            let base_type = typescript_rust_types::remove_undefined(&callee_type);

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
        ParsedExpression::Unknown => InferredExpression::Unknown,
    };
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.type_inference += infer_start.elapsed()
    });
    result
}

fn infer_arrow_function(
    arrow_function: &ParsedArrowFunction,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> typescript_rust_types::FunctionType {
    let parameters = arrow_function
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.declared_type.is_some() {
                Type::Any
            } else {
                match &parameter.binding_name {
                    typescript_rust_syntax::ParsedBindingName::Identifier { .. } => Type::Any,
                    typescript_rust_syntax::ParsedBindingName::ObjectPattern(_) => Type::Any,
                    typescript_rust_syntax::ParsedBindingName::Unsupported { .. } => Type::Any,
                }
            }
        })
        .collect::<Vec<_>>();

    let return_type = match &arrow_function.body {
        ParsedArrowFunctionBody::Expression(expression) => {
            match infer_expression(expression, symbols, ctx) {
                InferredExpression::Known(ty) => ty,
                _ => Type::Unknown,
            }
        }
        ParsedArrowFunctionBody::Block(_) => arrow_function
            .return_type
            .as_ref()
            .and_then(|ty| match ty {
                typescript_rust_syntax::ParsedType::String => Some(Type::String),
                typescript_rust_syntax::ParsedType::Number => Some(Type::Number),
                typescript_rust_syntax::ParsedType::Boolean => Some(Type::Boolean),
                typescript_rust_syntax::ParsedType::Any => Some(Type::Any),
                typescript_rust_syntax::ParsedType::Unknown => Some(Type::Unknown),
                typescript_rust_syntax::ParsedType::Undefined => Some(Type::Undefined),
                typescript_rust_syntax::ParsedType::Void => Some(Type::Void),
                _ => None,
            })
            .unwrap_or(Type::Unknown),
    };

    alloc_function_type(
        parameters,
        return_type,
        false,
        required_parameter_count(arrow_function.parameters.as_slice()),
    )
}

fn required_parameter_count(
    parameters: &[typescript_rust_syntax::ParsedFunctionParameter],
) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.initializer.is_some() {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

fn infer_unary_expression(
    operator: ParsedUnaryOperator,
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let operand_type = infer_expression(operand, symbols, ctx);

    match operator {
        ParsedUnaryOperator::Not => {
            if is_known_non_unknown(&operand_type) {
                InferredExpression::Known(Type::Boolean)
            } else {
                InferredExpression::Unknown
            }
        }
        ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus => match operand_type {
            InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Number),
            InferredExpression::Known(ty) if matches!(ty.base_primitive(), Some(Type::Number)) => {
                InferredExpression::Known(Type::Number)
            }
            InferredExpression::Known(Type::Unknown)
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown
            | InferredExpression::Known(Type::Undefined)
            | InferredExpression::Known(Type::Void)
            | InferredExpression::Known(Type::String)
            | InferredExpression::Known(Type::Number)
            | InferredExpression::Known(Type::Boolean)
            | InferredExpression::Known(Type::StringLiteral(_))
            | InferredExpression::Known(Type::NumberLiteral(_))
            | InferredExpression::Known(Type::BooleanLiteral(_))
            | InferredExpression::Known(Type::Object(_))
            | InferredExpression::Known(Type::Array(_))
            | InferredExpression::Known(Type::Tuple(_))
            | InferredExpression::Known(Type::Function(_))
            | InferredExpression::Known(Type::Union(_)) => InferredExpression::Unknown,
        },
    }
}

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

fn infer_array_literal(
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

fn infer_index_access(
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

                        if !typescript_rust_types::is_assignable_to(&index_type, &Type::Number) {
                            return InferredExpression::Unknown;
                        }
                        result_types.push(element_type.clone());
                    }
                    _ => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(typescript_rust_types::union_type(result_types))
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
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Undefined => InferredExpression::Unknown,
    }
}

fn infer_tuple_index_access(
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

pub(crate) fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}

fn infer_object_property_value(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    match infer_expression(parsed_expression, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        _ => Type::Unknown,
    }
}

fn infer_logical_expression(
    left: &ParsedExpression,
    right: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let left_type = infer_expression(left, symbols, ctx);
    let right_type = infer_expression(right, symbols, ctx);

    match (left_type, right_type) {
        (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
            if left_ty != Type::Unknown && right_ty != Type::Unknown =>
        {
            InferredExpression::Known(Type::Boolean)
        }
        _ => InferredExpression::Unknown,
    }
}

fn infer_conditional_expression(
    condition: &ParsedExpression,
    when_true: &ParsedExpression,
    when_false: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let condition_type = infer_expression(condition, symbols, ctx);
    if !is_known_non_unknown(&condition_type) {
        return InferredExpression::Unknown;
    }

    let true_type = infer_expression(when_true, symbols, ctx);
    let false_type = infer_expression(when_false, symbols, ctx);

    match (true_type, false_type) {
        (InferredExpression::Known(Type::Any), _) | (_, InferredExpression::Known(Type::Any)) => {
            InferredExpression::Known(Type::Any)
        }
        (InferredExpression::Known(true_ty), InferredExpression::Known(false_ty))
            if true_ty != Type::Unknown && false_ty != Type::Unknown =>
        {
            if true_ty == false_ty {
                InferredExpression::Known(true_ty)
            } else {
                InferredExpression::Known(union_type(vec![true_ty, false_ty]))
            }
        }
        _ => InferredExpression::Unknown,
    }
}

fn infer_binary_expression(
    operator: ParsedBinaryOperator,
    left: &ParsedExpression,
    right: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match operator {
        ParsedBinaryOperator::StrictEquals
        | ParsedBinaryOperator::StrictNotEquals
        | ParsedBinaryOperator::Equals
        | ParsedBinaryOperator::NotEquals
        | ParsedBinaryOperator::LessThan
        | ParsedBinaryOperator::LessThanEquals
        | ParsedBinaryOperator::GreaterThan
        | ParsedBinaryOperator::GreaterThanEquals => InferredExpression::Known(Type::Boolean),
        ParsedBinaryOperator::Add => {
            let left_type = infer_expression(left, symbols, ctx);
            let right_type = infer_expression(right, symbols, ctx);

            match (left_type, right_type) {
                (InferredExpression::Known(Type::Any), _)
                | (_, InferredExpression::Known(Type::Any)) => InferredExpression::Known(Type::Any),
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::String))
                        && matches!(right_ty.base_primitive(), Some(Type::String)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::String))
                        && matches!(right_ty.base_primitive(), Some(Type::Number)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::Number))
                        && matches!(right_ty.base_primitive(), Some(Type::String)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::Number))
                        && matches!(right_ty.base_primitive(), Some(Type::Number)) =>
                {
                    InferredExpression::Known(Type::Number)
                }
                _ => InferredExpression::Known(Type::Number),
            }
        }
        ParsedBinaryOperator::Subtract
        | ParsedBinaryOperator::Multiply
        | ParsedBinaryOperator::Divide
        | ParsedBinaryOperator::Remainder
        | ParsedBinaryOperator::Exponential
        | ParsedBinaryOperator::ShiftLeft
        | ParsedBinaryOperator::ShiftRight
        | ParsedBinaryOperator::ShiftRightZeroFill
        | ParsedBinaryOperator::BitwiseAnd
        | ParsedBinaryOperator::BitwiseOR
        | ParsedBinaryOperator::BitwiseXOR => InferredExpression::Known(Type::Number),
    }
}

fn infer_property_access(
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
            InferredExpression::Known(typescript_rust_types::union_type(result_types))
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

fn infer_property_call(
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
            InferredExpression::Known(typescript_rust_types::union_type(vec![
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
                    result_types.push(typescript_rust_types::union_type(vec![
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
            InferredExpression::Known(typescript_rust_types::union_type(result_types))
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

fn infer_new_expression(
    callee: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) =
            typescript_rust_types::Type::builtin_constructor_result_type(name)
    {
        return InferredExpression::Known(result_type);
    }

    match infer_expression(callee, symbols, ctx) {
        InferredExpression::Known(Type::Function(function_type)) => {
            InferredExpression::Known(function_type.return_type().clone())
        }
        InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Any),
        InferredExpression::UnresolvedIdentifier { name, span } => {
            InferredExpression::UnresolvedIdentifier { name, span }
        }
        _ => InferredExpression::Unknown,
    }
}

fn infer_optional_index_access(
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

    let base_type = typescript_rust_types::remove_undefined(&object_type);

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

fn infer_optional_property_access(
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

    let base_type = typescript_rust_types::remove_undefined(&object_type);

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
                InferredExpression::Known(typescript_rust_types::union_type(vec![
                    typescript_rust_types::union_type(result_types),
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

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if *ty != Type::Unknown)
}
