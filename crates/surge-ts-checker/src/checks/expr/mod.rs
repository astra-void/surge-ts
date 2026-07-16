mod diagnostics;
mod evaluate;
mod index_access;
mod inferred;

pub(crate) use diagnostics::*;
pub(crate) use evaluate::*;
use index_access::*;
pub(crate) use inferred::*;

use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedJsxChild, TextSpan as SyntaxTextSpan};
use surge_ts_types::{NumberLiteralType, Type, is_assignable_to, union_type};

use super::call::{
    check_call_like, check_new_like, check_optional_call_like, check_optional_property_call,
    check_property_call_like,
};
use super::emit_type_only_as_value_diagnostic;
use super::function::check_arrow_function_expression;
use super::ops;
use crate::arena::alloc_object_type;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::program::{record_expression_check, record_program_timing};
use crate::spans::{choose_span, diagnostic_with_syntax_span};
use crate::symbols::SymbolTable;
use surge_ts_types::{TypeCopyReason, with_type_copy_reason};

pub(crate) fn check_expression_statement(expression: ParsedExpression, ctx: &mut CheckerContext) {
    let start = Instant::now();
    let symbols = std::mem::take(&mut ctx.symbols);
    let _ = evaluate_expression(&expression, None, &symbols, ctx);
    ctx.symbols = symbols;
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.expression_statement_checking += start.elapsed()
    });
}

pub(crate) fn evaluate_const_expression(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let mut element_types = Vec::new();
            for element in elements {
                let inferred = evaluate_const_expression(
                    &element.expression,
                    element.span.or(fallback_span),
                    symbols,
                    ctx,
                );
                element_types.push(match inferred {
                    InferredExpression::Known(ty) => ty,
                    _ => Type::Unknown,
                });
            }
            let result = InferredExpression::Known(Type::Tuple(element_types));
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || result.clone()),
                fallback_span,
                symbols,
                ctx,
            );
            result
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            let mut props = surge_ts_types::PropertyMap::default();
            for property in properties {
                let inferred = evaluate_const_expression(
                    &property.value,
                    property.value_span.or(fallback_span),
                    symbols,
                    ctx,
                );
                let ty = match inferred {
                    InferredExpression::Known(ty) => ty,
                    _ => Type::Unknown,
                };
                props.insert(
                    property.name.clone(),
                    surge_ts_types::ObjectProperty {
                        ty,
                        optional: false,
                    },
                );
            }
            let result = InferredExpression::Known(Type::Object(alloc_object_type(props, None)));
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || result.clone()),
                fallback_span,
                symbols,
                ctx,
            );
            result
        }
        // Primitives just evaluate normally without widening
        _ => evaluate_expression(expression, fallback_span, symbols, ctx),
    }
}
