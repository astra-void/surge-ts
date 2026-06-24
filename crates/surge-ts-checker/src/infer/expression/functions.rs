//! Arrow-function expression inference.

use super::*;

use surge_ts_syntax::{ParsedArrowFunction, ParsedArrowFunctionBody};
use surge_ts_types::Type;

use crate::arena::alloc_function_type;
use crate::context::CheckerContext;
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_arrow_function(
    arrow_function: &ParsedArrowFunction,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> surge_ts_types::FunctionType {
    let parameters = arrow_function
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.declared_type.is_some() {
                Type::Any
            } else {
                match &parameter.binding_name {
                    surge_ts_syntax::ParsedBindingName::Identifier { .. } => Type::Any,
                    surge_ts_syntax::ParsedBindingName::ObjectPattern(_) => Type::Any,
                    surge_ts_syntax::ParsedBindingName::ArrayPattern(_) => Type::Any,
                    surge_ts_syntax::ParsedBindingName::Unsupported { .. } => Type::Any,
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
                surge_ts_syntax::ParsedType::String => Some(Type::String),
                surge_ts_syntax::ParsedType::Number => Some(Type::Number),
                surge_ts_syntax::ParsedType::Boolean => Some(Type::Boolean),
                surge_ts_syntax::ParsedType::Any => Some(Type::Any),
                surge_ts_syntax::ParsedType::Unknown => Some(Type::Unknown),
                surge_ts_syntax::ParsedType::UnknownKeyword => Some(Type::GenuineUnknown),
                surge_ts_syntax::ParsedType::Undefined => Some(Type::Undefined),
                surge_ts_syntax::ParsedType::Void => Some(Type::Void),
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
    parameters: &[surge_ts_syntax::ParsedFunctionParameter],
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
