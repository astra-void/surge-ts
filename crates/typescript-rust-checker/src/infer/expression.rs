use std::collections::BTreeMap;

use typescript_rust_syntax::{
    ParsedArrayElement, ParsedArrowFunction, ParsedArrowFunctionBody, ParsedBinaryOperator,
    ParsedExpression, ParsedObjectProperty, ParsedUnaryOperator, TextSpan,
};
use typescript_rust_types::{
    NumberLiteralType, ObjectProperty, ObjectType, Type, is_assignable_to, union_type,
};

use crate::symbols::SymbolTable;

use super::InferredExpression;

pub(crate) fn infer_expression(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
) -> InferredExpression {
    match parsed_expression {
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
        ParsedExpression::Identifier { name, span } => symbols
            .get(name)
            .map(|symbol| InferredExpression::Known(symbol.ty.clone()))
            .unwrap_or_else(|| InferredExpression::UnresolvedIdentifier {
                name: name.clone(),
                span: *span,
            }),
        ParsedExpression::ObjectLiteral { properties, .. } => {
            InferredExpression::Known(infer_object_literal(properties, symbols))
        }
        ParsedExpression::ArrayLiteral { elements, .. } => infer_array_literal(elements, symbols),
        ParsedExpression::Unary {
            operator, operand, ..
        } => infer_unary_expression(*operator, operand, symbols),
        ParsedExpression::Binary {
            operator,
            left,
            right,
            ..
        } => infer_binary_expression(*operator, left, right, symbols),
        ParsedExpression::Logical { left, right, .. } => {
            infer_logical_expression(left, right, symbols)
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => infer_conditional_expression(condition, when_true, when_false, symbols),
        ParsedExpression::PropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
        } => infer_property_access(object, object_span, property_name, property_span, symbols),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => infer_index_access(object_name, object_span, index, index_span, symbols),
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
        ),
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            let left_type = infer_expression(left, symbols);
            let right_type = infer_expression(right, symbols);

            match (left_type, right_type) {
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty)) => {
                    if left_ty == Type::Any || left_ty == Type::Unknown {
                        InferredExpression::Known(left_ty)
                    } else if left_ty == Type::Undefined {
                        InferredExpression::Known(right_ty)
                    } else {
                        let filtered_left = typescript_rust_types::remove_undefined(&left_ty);
                        InferredExpression::Known(union_type(vec![filtered_left, right_ty]))
                    }
                }
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::SatisfiesExpression { expression, .. } => {
            infer_expression(expression, symbols)
        }
        ParsedExpression::NonNullAssertion {
            expression,
            in_optional_chain,
            ..
        } => {
            let inferred = infer_expression(expression, symbols);
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
            infer_expression(expression, symbols)
        }
        ParsedExpression::ArrowFunction(arrow_function) => InferredExpression::Known(
            Type::Function(infer_arrow_function(arrow_function.as_ref(), symbols)),
        ),
        ParsedExpression::TypeAssertion {
            expression: _, ty, ..
        } => {
            // Primitive types can be inferred purely from AST
            match ty {
                typescript_rust_syntax::ParsedType::String => {
                    InferredExpression::Known(Type::String)
                }
                typescript_rust_syntax::ParsedType::Number => {
                    InferredExpression::Known(Type::Number)
                }
                typescript_rust_syntax::ParsedType::Boolean => {
                    InferredExpression::Known(Type::Boolean)
                }
                typescript_rust_syntax::ParsedType::Any => InferredExpression::Known(Type::Any),
                typescript_rust_syntax::ParsedType::Unknown => {
                    InferredExpression::Known(Type::Unknown)
                }
                typescript_rust_syntax::ParsedType::Undefined => {
                    InferredExpression::Known(Type::Undefined)
                }
                typescript_rust_syntax::ParsedType::Void => InferredExpression::Known(Type::Void),
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::Call { callee_name, .. } => match symbols.get(callee_name) {
            Some(symbol) => match &symbol.ty {
                Type::Function(function_type) => {
                    InferredExpression::Known((*function_type.return_type).clone())
                }
                Type::Unknown | Type::Any => InferredExpression::Unknown,
                _ => InferredExpression::Unknown,
            },
            None => InferredExpression::Unknown,
        },
        ParsedExpression::New { callee, .. } => infer_new_expression(callee, symbols),
        ParsedExpression::PropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            ..
        } => infer_property_call(object, object_span, property_name, property_span, symbols),
        ParsedExpression::OptionalPropertyCall {
            object,
            property_name,
            ..
        } => {
            let object_type = match infer_expression(object, symbols) {
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
                                *function_type.return_type.clone(),
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
            let callee_type = match infer_expression(callee, symbols) {
                InferredExpression::Known(ty) => ty,
                _ => return InferredExpression::Unknown,
            };

            let base_type = typescript_rust_types::remove_undefined(&callee_type);

            match base_type {
                Type::Function(function_type) => InferredExpression::Known(union_type(vec![
                    *function_type.return_type.clone(),
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
        } => infer_optional_index_access(object, object_span, index, index_span, symbols),
        ParsedExpression::Unknown => InferredExpression::Unknown,
    }
}

fn infer_arrow_function(
    arrow_function: &ParsedArrowFunction,
    symbols: &SymbolTable,
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
            match infer_expression(expression, symbols) {
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

    typescript_rust_types::FunctionType {
        parameters,
        return_type: Box::new(return_type),
        is_variadic: false,
        required_parameter_count: required_parameter_count(arrow_function.parameters.as_slice()),
    }
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
) -> InferredExpression {
    let operand_type = infer_expression(operand, symbols);

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
) -> Type {
    let properties = properties
        .iter()
        .map(|property| {
            (
                property.name.clone(),
                ObjectProperty::required(infer_object_property_value(&property.value, symbols)),
            )
        })
        .collect::<BTreeMap<_, _>>();

    Type::Object(ObjectType { properties })
}

fn infer_array_literal(
    elements: &[ParsedArrayElement],
    symbols: &SymbolTable,
) -> InferredExpression {
    if elements.is_empty() {
        return InferredExpression::Known(Type::Array(Box::new(Type::Any)));
    }

    let mut element_types = Vec::new();

    for element in elements {
        match infer_expression(&element.expression, symbols) {
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
            for ty in &union_type.types {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                match ty {
                    Type::Tuple(elements) => {
                        let res = infer_tuple_index_access(elements, index, index_span, symbols);
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

                        let index_type = match infer_expression(index, symbols) {
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
        Type::Tuple(elements) => infer_tuple_index_access(elements, index, index_span, symbols),
        Type::Array(element_type) => {
            let element_type = element_type.as_ref();
            if matches!(element_type, Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_type = match infer_expression(index, symbols) {
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
) -> InferredExpression {
    let index_type = match infer_expression(index, symbols) {
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
) -> Type {
    match infer_expression(parsed_expression, symbols) {
        InferredExpression::Known(ty) => ty,
        _ => Type::Unknown,
    }
}

fn infer_logical_expression(
    left: &ParsedExpression,
    right: &ParsedExpression,
    symbols: &SymbolTable,
) -> InferredExpression {
    let left_type = infer_expression(left, symbols);
    let right_type = infer_expression(right, symbols);

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
) -> InferredExpression {
    let condition_type = infer_expression(condition, symbols);
    if !is_known_non_unknown(&condition_type) {
        return InferredExpression::Unknown;
    }

    let true_type = infer_expression(when_true, symbols);
    let false_type = infer_expression(when_false, symbols);

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
            let left_type = infer_expression(left, symbols);
            let right_type = infer_expression(right, symbols);

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
        | ParsedBinaryOperator::Remainder => InferredExpression::Known(Type::Number),
    }
}

fn infer_property_access(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in &union_type.types {
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
    }
}

fn infer_property_call(
    object: &ParsedExpression,
    _object_span: &Option<TextSpan>,
    property_name: &str,
    _property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols) {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    match &object_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in &union_type.types {
                if *ty == Type::Undefined {
                    result_types.push(ty.clone());
                    continue;
                }
                match ty.get_property_access_type(property_name) {
                    Some(Type::Function(function_type)) => {
                        result_types.push((*function_type.return_type).clone());
                    }
                    Some(Type::Any) => result_types.push(Type::Any),
                    Some(_) | None => return InferredExpression::Unknown,
                }
            }
            InferredExpression::Known(typescript_rust_types::union_type(result_types))
        }
        _ => match object_type.get_property_access_type(property_name) {
            Some(Type::Function(function_type)) => {
                InferredExpression::Known((*function_type.return_type).clone())
            }
            Some(Type::Any) => InferredExpression::Known(Type::Any),
            Some(_) | None => InferredExpression::Unknown,
        },
    }
}

fn infer_new_expression(callee: &ParsedExpression, symbols: &SymbolTable) -> InferredExpression {
    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) =
            typescript_rust_types::Type::builtin_constructor_result_type(name)
    {
        return InferredExpression::Known(result_type);
    }

    match infer_expression(callee, symbols) {
        InferredExpression::Known(Type::Function(function_type)) => {
            InferredExpression::Known((*function_type.return_type).clone())
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
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols) {
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
            let result = infer_tuple_index_access(elements, index, index_span, symbols);
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

            let index_type = match infer_expression(index, symbols) {
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
) -> InferredExpression {
    let object_type = match infer_expression(object, symbols) {
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
        Type::Object(ref object_type) => {
            match object_type.get_property_access_type(property_name) {
                Some(ty) => InferredExpression::Known(ty),
                None => InferredExpression::MissingProperty {
                    property_name: property_name.to_string(),
                    object_type: base_type.clone(),
                    span: *property_span,
                },
            }
        }
        Type::Unknown | Type::Any => InferredExpression::Known(base_type.clone()),
        Type::Function(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::String
        | Type::Number
        | Type::Boolean
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Void
        | Type::Undefined => InferredExpression::Known(Type::Unknown),
        Type::Union(_) => InferredExpression::Known(Type::Unknown),
    };

    match result_type {
        InferredExpression::Known(ty) => {
            InferredExpression::Known(union_type(vec![ty, Type::Undefined]))
        }
        other => other,
    }
}

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if *ty != Type::Unknown)
}
