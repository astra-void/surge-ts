mod diagnostics;
mod evaluate;
mod guarded_unknown;
mod index_access;
mod inferred;

pub(crate) use diagnostics::*;
pub(crate) use evaluate::*;
pub(crate) use guarded_unknown::downgrade_guarded_genuine_unknown;
use guarded_unknown::downgrade_predicate_guarded_genuine_unknown;
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
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::metrics::alloc_object_type;
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

/// tsc reports a `readonly` array or tuple written to a mutable one with its
/// own code (`The_type_0_is_readonly_and_cannot_be_assigned_to_the_mutable_type_1`,
/// relater.go), not the generic assignability error.
pub(crate) fn readonly_to_mutable_mismatch(source: &Type, target: &Type) -> bool {
    let Type::Reference(reference) = source else {
        return false;
    };
    reference.is_readonly_array()
        && matches!(
            target,
            Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_)
        )
}

/// The diagnostic an assignability failure reports: TS4104 when a readonly
/// array or tuple is written to a mutable one, and otherwise the position's
/// ordinary code — TS2345 for an argument, TS2322 everywhere else.
pub(crate) fn assignability_mismatch_diagnostic(
    source: &Type,
    target: &Type,
    source_name: &str,
    target_name: &str,
    argument_position: bool,
    file_name: impl Into<String>,
) -> surge_ts_diagnostics::Diagnostic {
    let file_name = file_name.into();
    if readonly_to_mutable_mismatch(source, target) {
        return surge_ts_diagnostics::Diagnostic::ts4104(source_name, target_name, file_name);
    }
    if argument_position {
        surge_ts_diagnostics::Diagnostic::ts2345(source_name, target_name, file_name)
    } else {
        type_not_assignable_diagnostic(source, target, source_name, target_name, file_name)
    }
}

/// tsc's plain assignability message (`reportRelationError` with no head
/// message): TS2322, or TS2820 when a string literal missed a union by a typo
/// of one of its string-literal members.
pub(crate) fn type_not_assignable_diagnostic(
    source: &Type,
    target: &Type,
    source_name: &str,
    target_name: &str,
    file_name: impl Into<String>,
) -> surge_ts_diagnostics::Diagnostic {
    match suggested_string_literal_member(source, target) {
        Some(suggestion) => surge_ts_diagnostics::Diagnostic::ts2820(
            source_name,
            target_name,
            Type::StringLiteral(suggestion).name(),
            file_name,
        ),
        None => surge_ts_diagnostics::Diagnostic::ts2322(source_name, target_name, file_name),
    }
}

/// tsc's `getSuggestedTypeForNonexistentStringLiteralType`.
fn suggested_string_literal_member(source: &Type, target: &Type) -> Option<String> {
    let Type::StringLiteral(value) = source else {
        return None;
    };
    let peeled;
    let union = match target {
        Type::Union(union) => union,
        Type::Reference(_) => {
            peeled = target.peeled();
            let Type::Union(union) = &peeled else {
                return None;
            };
            union
        }
        _ => return None,
    };
    let members: Vec<Type> = union
        .types()
        .iter()
        .map(|member| match member {
            Type::Reference(_) => member.peeled(),
            other => other.clone(),
        })
        .collect();
    let candidates = members.iter().filter_map(|member| match member {
        Type::StringLiteral(candidate) => Some(candidate.as_str()),
        _ => None,
    });
    crate::checks::expr::inferred::spelling_suggestion(value, candidates, 1000)
        .map(str::to_string)
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
            // `as const` on an array makes it `readonly [...]`, which is what
            // makes a write through an index TS2540 rather than a type error.
            let result = InferredExpression::Known(
                crate::infer::types::readonly_reference(Type::Tuple(element_types)),
            );
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || result.clone()),
                fallback_span,
                symbols,
                ctx,
            );
            result
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            let properties = &*crate::infer::expression::resolve_computed_property_names(
                properties, symbols, ctx,
            );
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
                    property.name.as_str().into(),
                    surge_ts_types::ObjectProperty {
                        ty,
                        optional: false,
                        method: false,
                        // `as const` makes every property read-only, which is
                        // what turns a write to one into TS2540.
                        readonly: true,
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
