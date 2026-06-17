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

/// Recursively widens fresh literal types to their base primitive, descending
/// into object properties, array elements, and union members. This matches the
/// type tsc infers for `let`/`var` bindings (e.g. `let o = { a: 1 }` widens to
/// `{ a: number }`).
pub(crate) fn widen_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        // A named interface/type-alias object is not a fresh literal; preserve
        // it (and its alias name) as-is rather than widening its members.
        Type::Object(obj) if obj.alias_name.is_some() => ty.clone(),
        Type::Object(obj) => {
            let mut new_props = std::collections::BTreeMap::new();
            for (k, v) in obj.properties.iter() {
                new_props.insert(
                    k.clone(),
                    surge_ts_types::ObjectProperty {
                        ty: widen_type(&v.ty),
                        optional: v.optional,
                    },
                );
            }
            Type::Object(alloc_object_type(new_props, None))
        }
        Type::Array(inner) => Type::Array(Box::new(widen_type(inner))),
        Type::Union(types) => {
            let widened: Vec<_> = types.types().iter().map(widen_type).collect();
            surge_ts_types::union_type(widened)
        }
        _ => ty.clone(),
    }
}

/// `true` if `ty` is a literal type or a union containing one. tsc keeps the
/// source literal in assignability messages when the target is literal-like.
fn type_contains_literal(ty: &Type) -> bool {
    match ty {
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_) => true,
        Type::Union(types) => types.types().iter().any(type_contains_literal),
        _ => false,
    }
}

/// Type name for the SOURCE side of an assignment/argument diagnostic, matching
/// tsc: a fresh literal source is widened (`g(1)` to `string` -> `'number'`)
/// unless the target is literal-like, where tsc keeps the literal (`f("b")` to
/// `"a"` -> `'"b"'`).
/// Builds the diagnostic for a missing property access. When the object is a
/// class instance whose static side declares the property, tsc emits TS2576
/// ("Did you mean to access the static member ...") instead of the plain TS2339.
pub(crate) fn missing_property_diagnostic(
    property_name: &str,
    object_type: &Type,
    symbols: &SymbolTable,
    file_name: String,
) -> Diagnostic {
    let object_type_name = object_type.name();
    if let Some(class_name) =
        static_member_owner_for_missing_instance_property(property_name, object_type, symbols)
    {
        return Diagnostic::ts2576(property_name, &object_type_name, &class_name, file_name);
    }

    Diagnostic::ts2339(property_name, &object_type_name, file_name)
}

/// Returns the class name when `object_type` is a class instance (an object
/// tagged with the class name) and the class's static side declares
/// `property_name`, so the access should be reported as a static-member mixup.
fn static_member_owner_for_missing_instance_property(
    property_name: &str,
    object_type: &Type,
    symbols: &SymbolTable,
) -> Option<String> {
    let Type::Object(instance) = object_type else {
        return None;
    };
    let class_name = instance.alias_name.as_deref()?;
    let symbol = symbols.get(class_name)?;
    let Type::Object(static_side) = &symbol.ty else {
        return None;
    };
    if static_side.construct_signature().is_some()
        && static_side.get_property(property_name).is_some()
    {
        Some(class_name.to_string())
    } else {
        None
    }
}

pub(crate) fn source_display_name(source: &Type, target: &Type) -> String {
    if type_contains_literal(target) {
        source.name()
    } else {
        widen_type(source).name()
    }
}

/// Type name for an operand of an operator diagnostic (TS2365/TS2367), matching
/// tsc, which always widens fresh literal operands for display (e.g.
/// `1 === "string"` -> `'number'` and `'string'`).
pub(crate) fn operand_display_name(ty: &Type) -> String {
    widen_type(ty).name()
}

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
            let mut props = std::collections::BTreeMap::new();
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

pub(crate) fn evaluate_expression(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    record_expression_check();
    match expression {
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let inferred_expression = infer_expression(expression, symbols, ctx);

            for element in elements {
                let _ = evaluate_expression(
                    &element.expression,
                    element.span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }

            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    inferred_expression.clone()
                }),
                fallback_span,
                symbols,
                ctx,
            );
            inferred_expression
        }
        ParsedExpression::Call {
            callee_name,
            callee_span,
            type_arguments,
            arguments,
            ..
        } => match check_call_like(
            callee_name,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::New {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match check_new_like(
            callee,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::PropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            call_span,
            type_arguments,
            arguments,
            ..
        } => match check_property_call_like(
            object,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalPropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            call_span,
            type_arguments,
            arguments,
        } => match check_optional_property_call(
            object,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalCall {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match check_optional_call_like(
            callee,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::NullishCoalescing {
            left,
            left_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            match (left_result, right_result) {
                (InferredExpression::Known(left_type), InferredExpression::Known(right_type)) => {
                    if left_type == Type::Any || left_type == Type::Unknown {
                        InferredExpression::Known(left_type)
                    } else if left_type == Type::Undefined {
                        InferredExpression::Known(right_type)
                    } else {
                        let filtered_left = surge_ts_types::remove_nullish(&left_type);
                        InferredExpression::Known(union_type(vec![filtered_left, right_type]))
                    }
                }
                (InferredExpression::Known(Type::Unknown) | InferredExpression::Unknown, _)
                | (_, InferredExpression::Known(Type::Unknown) | InferredExpression::Unknown) => {
                    InferredExpression::Unknown
                }
                _ => InferredExpression::Unknown,
            }
        }
        ParsedExpression::Logical {
            left,
            left_span,
            operator: _,
            operator_span: _,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            ops::evaluate_logical_expression(left_result, right_result)
        }
        ParsedExpression::Binary {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            ops::evaluate_binary_expression(
                left_result,
                right_result,
                *operator,
                *left_span,
                *operator_span,
                *right_span,
                fallback_span,
                ctx,
            )
        }
        ParsedExpression::Unary {
            operator,
            operand,
            operand_span,
            ..
        } => {
            let operand_result =
                evaluate_expression(operand, operand_span.or(fallback_span), symbols, ctx);

            ops::evaluate_unary_expression(
                *operator,
                operand_result,
                operand_span.or(fallback_span),
                ctx,
            )
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
        } => {
            let condition_result =
                evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
            let true_result =
                evaluate_expression(when_true, when_true_span.or(fallback_span), symbols, ctx);
            let false_result =
                evaluate_expression(when_false, when_false_span.or(fallback_span), symbols, ctx);

            ops::evaluate_conditional_expression(condition_result, true_result, false_result)
        }
        ParsedExpression::OptionalPropertyAccess {
            object,
            object_span,
            property_name: _,
            property_span: _,
        } => {
            let _ = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
            let inferred_expression = infer_expression(expression, symbols, ctx);
            if let InferredExpression::MissingProperty {
                property_name,
                object_type,
                span,
            } = &inferred_expression
            {
                let diagnostic = missing_property_diagnostic(
                    property_name,
                    object_type,
                    symbols,
                    ctx.file_name.clone(),
                );
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(*span, fallback_span),
                ));
            }
            inferred_expression
        }
        ParsedExpression::OptionalIndexAccess {
            object,
            object_span,
            index,
            index_span,
        } => evaluate_optional_index_access(
            object,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => evaluate_index_access(
            object_name,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::SatisfiesExpression {
            expression: satisfied_expression,
            span,
            target_type,
            target_span: _,
        } => {
            let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
            let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
            let resolved_target_type =
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    crate::infer::map_parsed_type(target_type.clone(), ctx)
                });
            ctx.symbols = saved_symbols;

            // Evaluate the left expression contextually against the target type
            // This pushes contextual diagnostics (like excess properties, missing properties).
            let contextual_inferred =
                crate::checks::expected::evaluate_expression_with_expected_type(
                    satisfied_expression,
                    span.or(fallback_span),
                    Some(&resolved_target_type),
                    crate::checks::expected::ExpectedTypeDiagnostic::SatisfiesNotAssignable,
                    symbols,
                    ctx,
                );

            // We must also perform a top-level assignability check for things that don't do it
            // contextually (e.g. primitives, identifiers). However, if contextual checking already
            // failed and returned Unknown, we might get false cascades. Let's do a clean check
            // against the original inferred type.
            let original_inferred =
                crate::infer::infer_expression(satisfied_expression, symbols, ctx);

            // Check if contextual check already failed (meaning it returned Unknown when actual wasn't Unknown).
            let contextual_failed = matches!(
                contextual_inferred,
                crate::infer::InferredExpression::Unknown
            );
            let mut top_level_failed = false;

            if let crate::infer::InferredExpression::Known(actual_type) = &original_inferred {
                if *actual_type != surge_ts_types::Type::Unknown
                    && resolved_target_type != surge_ts_types::Type::Unknown
                {
                    let needs_top_level_check = !matches!(
                        satisfied_expression.as_ref(),
                        surge_ts_syntax::ParsedExpression::ObjectLiteral { .. }
                            | surge_ts_syntax::ParsedExpression::ArrayLiteral { .. }
                            | surge_ts_syntax::ParsedExpression::ConstAssertion { .. }
                            | surge_ts_syntax::ParsedExpression::Conditional { .. }
                    );

                    if needs_top_level_check
                        && !surge_ts_types::is_assignable_to(actual_type, &resolved_target_type)
                    {
                        top_level_failed = true;
                        let actual_type_name = actual_type.name();
                        let target_type_name = resolved_target_type.name();
                        let diagnostic = surge_ts_diagnostics::Diagnostic::ts1360(
                            &actual_type_name,
                            &target_type_name,
                            ctx.file_name.clone(),
                        );
                        let diagnostic = match span.or(fallback_span) {
                            Some(span) => diagnostic.with_span(crate::context::convert_span(span)),
                            None => diagnostic,
                        };
                        ctx.push(diagnostic);
                    }
                }
            }

            let final_inferred = match original_inferred {
                crate::infer::InferredExpression::Known(ty) => {
                    match ctx.options.diagnostic_profile {
                        crate::context::DiagnosticProfile::Tsc => {
                            if matches!(
                                **satisfied_expression,
                                ParsedExpression::ConstAssertion { .. }
                            ) {
                                crate::infer::InferredExpression::Known(ty)
                            } else {
                                crate::infer::InferredExpression::Known(widen_type(&ty))
                            }
                        }
                        crate::context::DiagnosticProfile::Native => {
                            crate::infer::InferredExpression::Known(ty)
                        }
                    }
                }
                other => other,
            };

            if contextual_failed {
                crate::infer::InferredExpression::Unknown
            } else if top_level_failed {
                match ctx.options.diagnostic_profile {
                    crate::context::DiagnosticProfile::Tsc => final_inferred,
                    crate::context::DiagnosticProfile::Native => {
                        crate::infer::InferredExpression::Unknown
                    }
                }
            } else {
                final_inferred
            }
        }
        ParsedExpression::TypeAssertion {
            expression: asserted_expression,
            expression_span,
            ty,
            type_span: _,
        } => {
            // Evaluate the inner expression so it participates in checking
            let _ = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );

            let temp_symbols = symbols.clone_with_reason(TypeCopyReason::ExpressionInference);
            let saved_symbols = std::mem::replace(&mut ctx.symbols, temp_symbols);
            let resolved_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                crate::infer::map_parsed_type(ty.clone(), ctx)
            });
            ctx.symbols = saved_symbols;

            // If the type is unresolved (e.g. unknown named type), map_parsed_type
            // already emits TS2304 and returns Type::Unknown.
            // We just return it as the assertion result.
            InferredExpression::Known(resolved_type)
        }
        ParsedExpression::ConstAssertion {
            expression: asserted_expression,
            span: expression_span,
        } => evaluate_const_expression(
            asserted_expression,
            expression_span.or(fallback_span),
            symbols,
            ctx,
        ),
        ParsedExpression::ArrowFunction(arrow_function) => {
            let function_type = with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                check_arrow_function_expression(arrow_function.as_ref().clone(), symbols, ctx)
            });
            InferredExpression::Known(Type::Function(function_type))
        }
        ParsedExpression::NonNullAssertion {
            expression: asserted_expression,
            span: expression_span,
            in_optional_chain,
        } => {
            let inferred = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );

            match inferred {
                InferredExpression::Known(ty) => {
                    let filtered = surge_ts_types::remove_undefined(&ty);
                    if *in_optional_chain {
                        InferredExpression::Known(surge_ts_types::union_type(vec![
                            filtered,
                            surge_ts_types::Type::Undefined,
                        ]))
                    } else {
                        InferredExpression::Known(filtered)
                    }
                }
                other => other,
            }
        }
        ParsedExpression::JsxElement {
            tag_name,
            tag_name_span,
            component_name,
            component_span,
            attributes,
            children,
            span,
        } => {
            super::jsx::check_jsx_element(
                tag_name,
                *tag_name_span,
                component_name.as_deref(),
                *component_span,
                *span,
                attributes,
                children,
                fallback_span,
                symbols,
                ctx,
            );

            infer_expression(expression, symbols, ctx)
        }
        ParsedExpression::JsxFragment { children, .. } => {
            for child in children {
                evaluate_jsx_child(child, fallback_span, symbols, ctx);
            }

            infer_expression(expression, symbols, ctx)
        }
        _ => {
            let inferred_expression = infer_expression(expression, symbols, ctx);
            report_inferred_expression(
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    inferred_expression.clone()
                }),
                fallback_span,
                symbols,
                ctx,
            );
            inferred_expression
        }
    }
}

/// Walks a JSX child for ordinary diagnostics. Text is inert; `{expression}`
/// containers and nested elements are evaluated so diagnostics such as unresolved
/// names inside `{...}` are still reported.
fn evaluate_jsx_child(
    child: &ParsedJsxChild,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    match child {
        ParsedJsxChild::Text => {}
        ParsedJsxChild::Expression { expression, span } => {
            if let Some(expression) = expression {
                let _ = evaluate_expression(expression, span.or(fallback_span), symbols, ctx);
            }
        }
        ParsedJsxChild::Element(element) => {
            let _ = evaluate_expression(element, fallback_span, symbols, ctx);
        }
    }
}

pub(crate) fn report_inferred_expression(
    inferred_expression: InferredExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    match inferred_expression {
        InferredExpression::Known(known_type) => {
            if known_type == Type::Unknown {
                return;
            }
        }
        InferredExpression::UnresolvedIdentifier { name, span } => {
            if emit_type_only_as_value_diagnostic(&name, span, ctx) {
                return;
            }

            if is_missing_node_like_global(&name, ctx) {
                let diagnostic = if ctx.options.types_uses_wildcard() {
                    Diagnostic::ts2580(&name, ctx.file_name.clone())
                } else {
                    Diagnostic::ts2591(&name, ctx.file_name.clone())
                };
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(span, fallback_span),
                ));
                return;
            }

            if let Some(suggestion) = suggested_unresolved_name(&name, ctx) {
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts2552(&name, suggestion, ctx.file_name.clone()),
                    choose_span(span, fallback_span),
                ));
                return;
            }

            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2304(&name, ctx.file_name.clone()),
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::MissingProperty {
            property_name,
            object_type,
            span,
        } => {
            let diagnostic = missing_property_diagnostic(
                &property_name,
                &object_type,
                symbols,
                ctx.file_name.clone(),
            );
            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::Unknown => {}
    }
}

fn suggested_unresolved_name(name: &str, ctx: &CheckerContext) -> Option<String> {
    let mut candidates = ctx
        .symbols
        .iter()
        .chain(ctx.ambient_global_symbols.iter())
        .map(|(candidate, _)| candidate.as_ref())
        .filter(|candidate| candidate.eq_ignore_ascii_case(name) && *candidate != name);

    candidates.next().map(|candidate| candidate.to_string())
}

fn is_missing_node_like_global(name: &str, ctx: &CheckerContext) -> bool {
    if ctx.options.types.iter().any(|ty| ty == "node") {
        return false;
    }

    matches!(name, "Buffer" | "process")
}

fn evaluate_optional_index_access(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_result = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);

    let object_type = match object_result {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = surge_ts_types::remove_undefined(&object_type);

    match base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = crate::infer::tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => {
                        InferredExpression::Known(union_type(vec![element_type, Type::Undefined]))
                    }
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                union_type(elements.to_vec()),
                Type::Undefined,
            ]))
        }
        Type::Array(element_type) => {
            if matches!(element_type.as_ref(), Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                with_type_copy_reason(TypeCopyReason::ExpressionInference, || {
                    element_type.as_ref().clone()
                }),
                Type::Undefined,
            ]))
        }
        _ => InferredExpression::Unknown,
    }
}

fn evaluate_index_access(
    object_name: &str,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name) else {
        if emit_type_only_as_value_diagnostic(object_name, object_span, ctx) {
            return InferredExpression::Unknown;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(object_name, ctx.file_name.clone()),
            choose_span(object_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    };

    match &symbol.ty {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => InferredExpression::Known(element_type),
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(elements.to_vec()))
        }
        Type::Array(element_type) => {
            if matches!(element_type.as_ref(), Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(with_type_copy_reason(
                TypeCopyReason::ExpressionInference,
                || element_type.as_ref().clone(),
            ))
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
        | Type::Undefined
        | Type::Union(_) => {
            let object_type_name = symbol.ty.name();
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone()),
                choose_span(object_span, fallback_span),
            ));
            InferredExpression::Unknown
        }
    }
}

fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}
