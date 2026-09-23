// Withheld, not deleted: the rule and its fixture are correct, what is not
// good enough is surge's structural expansion of a library's generic types.
// See `tests/compat-projects/assertion-overlap-basic/README.md`.
#[allow(dead_code)]
mod accessibility;
mod assertion;
mod diagnostics;
mod evaluate;
mod guarded_unknown;
mod index_access;
mod inferred;
mod lib_features;
mod operand_types;
mod operand_writes;
mod unresolved;

pub(crate) use accessibility::{ClassIdentity, check_member_accessibility, enclosing_class_lineage};
pub(crate) use diagnostics::*;
pub(crate) use evaluate::*;
pub(crate) use guarded_unknown::downgrade_guarded_genuine_unknown;
use guarded_unknown::downgrade_predicate_guarded_genuine_unknown;
use index_access::*;
pub(crate) use index_access::object_element_read;
pub(crate) use inferred::*;
pub(crate) use lib_features::{lib_feature_of_missing_member, suggested_lib_for_nonexistent_name};
pub(crate) use operand_types::{
    check_instanceof_left_operand, check_instanceof_right_operand, check_iterable_operand,
    check_object_spread_type,
    is_definitely_not_iterable,
};
pub(crate) use operand_writes::{check_delete_operand, check_update_operand, update_result_type};
pub(crate) use unresolved::{
    EnclosingClassMembers, UnresolvedNameSite, cannot_find_name_message,
    export_assignment_target_is_exempt, report_unresolved_value_name,
    unresolved_type_query_diagnostic,
};

use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedJsxChild, TextSpan as SyntaxTextSpan};
use surge_ts_types::{NumberLiteralType, Type, is_assignable_to, union_type};

use super::call::{
    check_call_like, check_new_like, check_optional_call_like, check_optional_property_call,
    check_property_call_like,
};
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
        argument_not_assignable_diagnostic(source, target, source_name, target_name, file_name)
    } else {
        type_not_assignable_diagnostic(source, target, source_name, target_name, file_name)
    }
}

/// TS2345, or the missing-property report that replaces it (see
/// [`missing_properties_report`]).
pub(crate) fn argument_not_assignable_diagnostic(
    source: &Type,
    target: &Type,
    source_name: &str,
    target_name: &str,
    file_name: impl Into<String>,
) -> surge_ts_diagnostics::Diagnostic {
    let file_name = file_name.into();
    missing_properties_report(source, target, source_name, target_name, &file_name)
        .unwrap_or_else(|| {
            surge_ts_diagnostics::Diagnostic::ts2345(source_name, target_name, file_name)
        })
}

/// tsc's `reportRelationError` suppresses its own head (TS2322, TS2345) when
/// the chain beneath it is a missing-property report for the same pair, and
/// `propertiesRelatedTo` looks for a missing property before it compares any
/// property's type. So an object source that lacks a required member of an
/// object target reports TS2741 for one missing member, TS2739 for up to five
/// and TS2740 beyond, whatever else is wrong with it.
pub(crate) fn missing_properties_report(
    source: &Type,
    target: &Type,
    source_name: &str,
    target_name: &str,
    file_name: &str,
) -> Option<surge_ts_diagnostics::Diagnostic> {
    let missing = missing_required_properties(source, target)?;
    let first = missing.first()?.clone();
    Some(missing_properties_diagnostic(
        &first,
        &missing,
        source_name,
        target_name,
        file_name,
    ))
}

/// The members of the global `Array` an object source lacks, in the order the
/// lib declares them (tsc lists the first four). The count beyond those follows
/// surge's own table of array members, not the configured lib's.
fn missing_array_members(source: &Type) -> Option<Vec<String>> {
    const DECLARATION_ORDER: [&str; 20] = [
        "length", "pop", "push", "concat", "join", "reverse", "shift", "slice", "sort", "splice",
        "unshift", "indexOf", "lastIndexOf", "every", "some", "forEach", "map", "filter", "reduce",
        "reduceRight",
    ];
    let Type::Object(object) = source else {
        return None;
    };
    if object.is_intersection
        || object.synthetic_open_index
        || object.non_primitive
        || object.call_signature().is_some()
        || object.construct_signature().is_some()
        || object.alias_name.as_deref() == Some("Object")
    {
        return None;
    }
    let missing: Vec<String> = DECLARATION_ORDER
        .iter()
        .chain(
            surge_ts_types::array_property_names()
                .iter()
                .filter(|name| !DECLARATION_ORDER.contains(name)),
        )
        .filter(|name| {
            !object.properties.contains_key(**name)
                && surge_ts_types::object_prototype_member_type(name).is_none()
        })
        .map(|name| name.to_string())
        .collect();
    (!missing.is_empty()).then_some(missing)
}

fn missing_required_properties(source: &Type, target: &Type) -> Option<Vec<String>> {
    // Two instantiations of one generic relate through their type arguments,
    // so any missing member is reported for an argument pair, beneath the head.
    if let (Type::Reference(source), Type::Reference(target)) = (source, target)
        && source.id == target.id
    {
        return None;
    }
    let source = source.peeled();
    let target = target.peeled();
    if let Type::Array(_) = &target {
        return missing_array_members(&source);
    }
    let Type::Object(target_object) = &target else {
        return None;
    };
    if target_object.is_intersection
        || target_object.synthetic_open_index
        || target_object.properties.values().any(|property| property.ty.is_unknown())
    {
        return None;
    }
    let has_property = |name: &str| -> bool {
        match &source {
            Type::Object(object) => {
                object.properties.contains_key(name)
                    || surge_ts_types::object_prototype_member_type(name).is_some()
                    || ((object.call_signature().is_some() || object.construct_signature().is_some())
                        && source.get_property_access_type(name).is_some())
            }
            _ => source.get_property_access_type(name).is_some(),
        }
    };
    // An intersection with a primitive operand (`number & { __brand: T }`)
    // relates through its apparent type, so tsc names the missing members
    // beneath the plain head rather than as it. One of object types alone is
    // reported like any object.
    let relates_through_apparent_type = |object: &surge_ts_types::ObjectType| {
        object.is_intersection
            && object.intersection_operands.as_deref().is_some_and(|operands| {
                operands
                    .iter()
                    .any(|operand| !matches!(operand, Type::Reference(_) | Type::Object(_)))
            })
    };
    // A function source keeps the plain head, as does the global `Object`
    // (tsc chains its "assignable to very few other types" hint beneath it).
    match &source {
        Type::Object(object)
            if !relates_through_apparent_type(object)
                && !object.synthetic_open_index
                && object.alias_name.as_deref() != Some("Object")
                && !object.non_primitive
                && (object.call_signature().is_none() && object.construct_signature().is_none()
                    || !object.properties.is_empty()) => {}
        Type::Array(_) | Type::Tuple(_) => {}
        _ => return None,
    }
    let missing: Vec<String> = target_object
        .required_properties()
        .filter(|(name, _)| !has_property(name))
        .map(|(name, _)| name.to_string())
        .collect();
    (!missing.is_empty()).then_some(missing)
}

/// tsc names *every* missing required property, and picks the code by how many
/// there are: one is TS2741, two to five are listed in full as TS2739, and six
/// or more list the first four as TS2740 with the rest counted.
pub(crate) fn missing_properties_diagnostic(
    first_missing: &str,
    missing: &[String],
    source_type_name: &str,
    target_type_name: &str,
    file_name: &str,
) -> surge_ts_diagnostics::Diagnostic {
    use surge_ts_diagnostics::Diagnostic;
    const LISTED_WHEN_TRUNCATED: usize = 4;
    const MAX_LISTED: usize = 5;

    match missing.len() {
        0 | 1 => Diagnostic::ts2741(first_missing, source_type_name, target_type_name, file_name),
        count if count <= MAX_LISTED => {
            Diagnostic::ts2739(source_type_name, target_type_name, missing.join(", "), file_name)
        }
        count => Diagnostic::ts2740(
            source_type_name,
            target_type_name,
            missing[..LISTED_WHEN_TRUNCATED].join(", "),
            count - LISTED_WHEN_TRUNCATED,
            file_name,
        ),
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
    let file_name = file_name.into();
    if let Some(diagnostic) =
        missing_properties_report(source, target, source_name, target_name, &file_name)
    {
        return diagnostic;
    }
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
                        restriction: None,
                        index_slot: false,
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
        // A template in a const context is typed by its own pattern (tsc's
        // `checkTemplateExpression`); its interpolations are still checked as
        // the expressions they are.
        ParsedExpression::TemplateLiteral {
            expressions,
            quasis,
            ..
        } if !expressions.is_empty() => {
            let evaluated = evaluate_expression(expression, fallback_span, symbols, ctx);
            match crate::infer::expression::template_expression_pattern_type(
                expressions,
                quasis,
                symbols,
                ctx,
            ) {
                Some(pattern) => InferredExpression::Known(pattern),
                None => evaluated,
            }
        }
        // Primitives just evaluate normally without widening
        _ => evaluate_expression(expression, fallback_span, symbols, ctx),
    }
}
