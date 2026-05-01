use std::collections::BTreeMap;

use typescript_rust_syntax::{
    ParsedBinaryOperator, ParsedExpression, ParsedObjectProperty, ParsedType, ParsedUnaryOperator,
    TextSpan,
};
use typescript_rust_types::{ObjectType, Type};

use crate::symbols::SymbolTable;

#[derive(Debug, Clone)]
pub(crate) enum InferredExpression {
    Known(Type),
    UnresolvedIdentifier {
        name: String,
        span: Option<TextSpan>,
    },
    MissingProperty {
        property_name: String,
        object_type: Type,
        span: Option<TextSpan>,
    },
    Unknown,
}

pub(crate) fn map_parsed_type(parsed_type: ParsedType) -> Type {
    match parsed_type {
        ParsedType::String => Type::String,
        ParsedType::Number => Type::Number,
        ParsedType::Boolean => Type::Boolean,
        ParsedType::Any => Type::Any,
        ParsedType::Unknown => Type::Unknown,
    }
}

pub(crate) fn infer_expression(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
) -> InferredExpression {
    match parsed_expression {
        ParsedExpression::StringLiteral => InferredExpression::Known(Type::String),
        ParsedExpression::NumberLiteral => InferredExpression::Known(Type::Number),
        ParsedExpression::BooleanLiteral => InferredExpression::Known(Type::Boolean),
        ParsedExpression::Identifier(name) => symbols
            .get(name)
            .map(|symbol| InferredExpression::Known(symbol.ty.clone()))
            .unwrap_or_else(|| InferredExpression::UnresolvedIdentifier {
                name: name.clone(),
                span: None,
            }),
        ParsedExpression::ObjectLiteral(properties) => {
            InferredExpression::Known(infer_object_literal(properties, symbols))
        }
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
            object_name,
            object_span,
            property_name,
            property_span,
        } => infer_property_access(
            object_name,
            object_span,
            property_name,
            property_span,
            symbols,
        ),
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
        ParsedExpression::Unknown => InferredExpression::Unknown,
    }
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
            InferredExpression::Known(Type::Number) => InferredExpression::Known(Type::Number),
            InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Number),
            InferredExpression::Known(Type::Unknown)
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown
            | InferredExpression::Known(Type::String)
            | InferredExpression::Known(Type::Boolean)
            | InferredExpression::Known(Type::Object(_))
            | InferredExpression::Known(Type::Function(_)) => InferredExpression::Unknown,
        },
    }
}

fn infer_object_literal(properties: &[ParsedObjectProperty], symbols: &SymbolTable) -> Type {
    let properties = properties
        .iter()
        .map(|property| {
            (
                property.name.clone(),
                infer_object_property_value(&property.value, symbols),
            )
        })
        .collect::<BTreeMap<_, _>>();

    Type::Object(ObjectType { properties })
}

fn infer_object_property_value(
    parsed_expression: &ParsedExpression,
    symbols: &SymbolTable,
) -> Type {
    match parsed_expression {
        ParsedExpression::StringLiteral => Type::String,
        ParsedExpression::NumberLiteral => Type::Number,
        ParsedExpression::BooleanLiteral => Type::Boolean,
        ParsedExpression::Identifier(name) => symbols
            .get(name)
            .map(|symbol| symbol.ty.clone())
            .unwrap_or(Type::Unknown),
        ParsedExpression::Binary {
            operator,
            left,
            right,
            ..
        } => match infer_binary_expression(*operator, left, right, symbols) {
            InferredExpression::Known(ty) => ty,
            _ => Type::Unknown,
        },
        ParsedExpression::Logical { left, right, .. } => {
            match infer_logical_expression(left, right, symbols) {
                InferredExpression::Known(ty) => ty,
                _ => Type::Unknown,
            }
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => match infer_conditional_expression(condition, when_true, when_false, symbols) {
            InferredExpression::Known(ty) => ty,
            _ => Type::Unknown,
        },
        ParsedExpression::Unary { .. } => match infer_expression(parsed_expression, symbols) {
            InferredExpression::Known(ty) => ty,
            _ => Type::Unknown,
        },
        ParsedExpression::Unknown => Type::Unknown,
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
            if true_ty != Type::Unknown && false_ty != Type::Unknown && true_ty == false_ty =>
        {
            InferredExpression::Known(true_ty)
        }
        (InferredExpression::Known(true_ty), InferredExpression::Known(false_ty))
            if true_ty != Type::Unknown && false_ty != Type::Unknown =>
        {
            InferredExpression::Unknown
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
                (
                    InferredExpression::Known(Type::String),
                    InferredExpression::Known(Type::String),
                )
                | (
                    InferredExpression::Known(Type::String),
                    InferredExpression::Known(Type::Number),
                )
                | (
                    InferredExpression::Known(Type::Number),
                    InferredExpression::Known(Type::String),
                ) => InferredExpression::Known(Type::String),
                (
                    InferredExpression::Known(Type::Number),
                    InferredExpression::Known(Type::Number),
                ) => InferredExpression::Known(Type::Number),
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
    object_name: &str,
    object_span: &Option<TextSpan>,
    property_name: &str,
    property_span: &Option<TextSpan>,
    symbols: &SymbolTable,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name) else {
        return InferredExpression::UnresolvedIdentifier {
            name: object_name.to_string(),
            span: *object_span,
        };
    };

    match &symbol.ty {
        Type::Object(object_type) => object_type
            .properties
            .get(property_name)
            .cloned()
            .map(InferredExpression::Known)
            .unwrap_or_else(|| InferredExpression::MissingProperty {
                property_name: property_name.to_string(),
                object_type: symbol.ty.clone(),
                span: *property_span,
            }),
        Type::Unknown | Type::Any => InferredExpression::Unknown,
        Type::Function(_) | Type::String | Type::Number | Type::Boolean => {
            InferredExpression::MissingProperty {
                property_name: property_name.to_string(),
                object_type: symbol.ty.clone(),
                span: *property_span,
            }
        }
    }
}

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if *ty != Type::Unknown)
}
