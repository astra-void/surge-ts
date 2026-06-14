//! Arrow-function expression inference.

use super::*;

use typescript_rust_syntax::{ParsedArrowFunction, ParsedArrowFunctionBody};
use typescript_rust_types::Type;

use crate::arena::alloc_function_type;
use crate::context::CheckerContext;
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_arrow_function(
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

pub(crate) fn required_parameter_count(
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
