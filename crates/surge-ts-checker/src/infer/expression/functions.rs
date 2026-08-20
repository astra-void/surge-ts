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
        ParsedArrowFunctionBody::Block(body) => arrow_function
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
            .or_else(|| {
                infer_block_body_return_type(body, &arrow_function.parameters, symbols, ctx)
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

/// Infers an unannotated block-bodied arrow's return type from its `return`
/// statements. Deliberately narrow: a run of local declarations followed by
/// returns is the shape a callback argument almost always has, and it is what
/// lets a lazy initializer (`useState(() => { const rows = …; return rows; })`)
/// contribute its type to the call's inference. Anything with branching, a bare
/// `return;`, or a binding pattern yields `None`, keeping the previous
/// `Unknown` — the body would need real flow analysis to type honestly.
fn infer_block_body_return_type(
    body: &[surge_ts_syntax::ParsedFunctionBodyStatement],
    parameters: &[surge_ts_syntax::ParsedFunctionParameter],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    use surge_ts_syntax::{ParsedBindingName, ParsedFunctionBodyStatement};

    let returns_here = body
        .iter()
        .any(|statement| matches!(statement, ParsedFunctionBodyStatement::Return(_)));
    if !returns_here {
        return None;
    }
    // Any statement that can carry control flow (or a return this walk would
    // miss) disqualifies the body.
    let straight_line = body.iter().all(|statement| {
        matches!(
            statement,
            ParsedFunctionBodyStatement::VariableDeclaration(_)
                | ParsedFunctionBodyStatement::Return(_)
                | ParsedFunctionBodyStatement::Expression(_)
                | ParsedFunctionBodyStatement::TypeAlias(_)
        )
    });
    if !straight_line {
        return None;
    }

    let mut locals = symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    for parameter in parameters {
        let ParsedBindingName::Identifier { name, .. } = &parameter.binding_name else {
            return None;
        };
        let _ = locals.insert(
            name.clone(),
            crate::symbols::SymbolInfo {
                ty: Type::Any,
                kind: crate::symbols::SymbolKind::Parameter,
                function_signature: None,
            },
        );
    }

    let mut returned = Vec::new();
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
                if variable.declared_type.is_some() {
                    return None;
                }
                let initializer = variable.initializer.as_ref()?;
                let InferredExpression::Known(ty) = infer_expression(initializer, &locals, ctx)
                else {
                    return None;
                };
                let _ = locals.insert(
                    variable.name.clone(),
                    crate::symbols::SymbolInfo {
                        ty,
                        kind: match variable.kind {
                            surge_ts_syntax::ParsedVariableKind::Var => {
                                crate::symbols::SymbolKind::Var
                            }
                            surge_ts_syntax::ParsedVariableKind::Let => {
                                crate::symbols::SymbolKind::Let
                            }
                            surge_ts_syntax::ParsedVariableKind::Const => {
                                crate::symbols::SymbolKind::Const
                            }
                        },
                        function_signature: None,
                    },
                );
            }
            ParsedFunctionBodyStatement::Return(statement) => {
                let expression = statement.expression.as_ref()?;
                let InferredExpression::Known(ty) = infer_expression(expression, &locals, ctx)
                else {
                    return None;
                };
                if ty.is_unknown() {
                    return None;
                }
                returned.push(ty);
            }
            _ => {}
        }
    }

    match returned.len() {
        0 => None,
        1 => returned.pop(),
        _ => Some(surge_ts_types::union_type(returned)),
    }
}
