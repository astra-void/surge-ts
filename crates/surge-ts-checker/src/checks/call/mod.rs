use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCall, ParsedCallArgument, ParsedExpression, ParsedNamedType, ParsedType,
    TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{
    FunctionType, Type, TypeCopyReason, UnionType, is_assignable_to, union_type,
    with_type_copy_reason,
};

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::{evaluate_expression, source_display_name};
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::program::{record_call_resolution, record_program_timing};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

mod builtins;
mod instantiate;
mod property;

pub(crate) use builtins::*;
pub(crate) use instantiate::*;
pub(crate) use property::*;
pub(crate) fn check_call(call: ParsedCall, ctx: &mut CheckerContext) {
    let symbols = ctx
        .symbols
        .clone_with_reason(TypeCopyReason::CallResolution);
    let _ = check_call_like(
        &call.callee_name,
        call.callee_span,
        call.span,
        &call.type_arguments,
        &call.arguments,
        &symbols,
        ctx,
    );
}

pub(crate) fn check_call_like(
    callee_name: &str,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    record_call_resolution();
    let call_start = Instant::now();
    let Some(symbol) = symbols.get(callee_name) else {
        if emit_type_only_as_value_diagnostic(callee_name, callee_span, ctx) {
            return None;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(callee_name, ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    if matches!(symbol.ty, Type::Unknown) {
        return None;
    }

    let result = match &symbol.ty {
        Type::Function(function_type) => {
            with_type_copy_reason(TypeCopyReason::CallResolution, || {
                let function_type = instantiate_function_type(
                    function_type,
                    symbol.function_signature.as_ref(),
                    type_arguments,
                    callee_span,
                    arguments,
                    symbols,
                    ctx,
                );

                check_function_type_call(
                    function_type.as_ref(),
                    callee_span,
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                )
            })
        }
        Type::Union(union) => check_callable_union_call(
            union,
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ),
        Type::Object(object_type) if object_type.call_signature().is_some() => {
            let call_signature = object_type.call_signature().unwrap();
            with_type_copy_reason(TypeCopyReason::CallResolution, || {
                check_function_type_call(
                    call_signature,
                    callee_span,
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                )
            })
        }
        _ => {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2349(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    };

    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.call_expression_checking += call_start.elapsed()
    });
    result
}

/// Phase 1 callable-union calls: a union is callable when every member is a
/// function type sharing one call signature (identical arity and pairwise
/// mutually-assignable parameters). Return types may differ and are unified into
/// the call result. An unresolved member already reported upstream suppresses the
/// call cascade; any other non-callable union is pinned as TS2349.
fn check_callable_union_call(
    union: &UnionType,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if union.types().iter().any(|ty| matches!(ty, Type::Unknown)) {
        return None;
    }

    let Some(members) = shared_signature_function_members(union) else {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2349(ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    let representative = members[0];
    let return_types = members
        .iter()
        .map(|member| member.return_type().clone())
        .collect::<Vec<_>>();

    with_type_copy_reason(TypeCopyReason::CallResolution, || {
        check_function_type_call(
            representative,
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
        .map(|_| union_type(return_types))
    })
}

/// Returns the function members of a union when every member is a function type
/// that shares one Phase 1 call signature, or `None` when the union is not
/// callable under Phase 1 rules (a non-function member, mismatched arity, or
/// parameters that are not mutually assignable). Return-type differences are
/// permitted and unified by the caller.
fn shared_signature_function_members(union: &UnionType) -> Option<Vec<&FunctionType>> {
    let mut members = Vec::with_capacity(union.types().len());
    for ty in union.types() {
        match ty {
            Type::Function(function_type) => members.push(function_type),
            _ => return None,
        }
    }

    let first = members.first()?;
    let shares_signature = members.iter().all(|member| {
        member.parameters().len() == first.parameters().len()
            && member.required_parameter_count() == first.required_parameter_count()
            && member.is_variadic() == first.is_variadic()
            && member
                .parameters()
                .iter()
                .zip(first.parameters().iter())
                .all(|(member_parameter, first_parameter)| {
                    is_assignable_to(member_parameter, first_parameter)
                        && is_assignable_to(first_parameter, member_parameter)
                })
    });

    shares_signature.then_some(members)
}

pub(crate) fn check_new_like(
    callee: &ParsedExpression,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    // Physical-lib mode: `new Foo<Args>()` produces an instance of the `Foo`
    // interface (the instance interface shares the constructor's name, e.g.
    // `Map<K, V>`, `Date`, `URL`, `Response`). Prefer resolving the real
    // interface instance over the hardcoded builtin fast-path so that lib
    // methods and properties carry meaningful types. Gated to interfaces
    // declared in physical default-lib files, so generated/default mode keeps
    // the existing builtin behaviour.
    if let ParsedExpression::Identifier { name, .. } = callee {
        let physical_interface_file = match ctx.lookup_type_declaration(name) {
            Some(crate::symbols::TypeDeclarationInfo::Interface(info)) => {
                if crate::default_lib::is_physical_default_lib_file_name(&info.file_name) {
                    Some(())
                } else {
                    None
                }
            }
            _ => None,
        };
        if physical_interface_file.is_some() {
            for argument in arguments {
                let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            }
            let named = ParsedType::Named(ParsedNamedType {
                name: name.clone(),
                span: None,
                type_arguments: type_arguments.to_vec(),
            });
            return Some(crate::infer::map_parsed_type(named, ctx));
        }
    }

    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) = surge_ts_types::Type::builtin_constructor_result_type(name)
    {
        for argument in arguments {
            let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
        }
        return Some(result_type);
    }

    let callee_result = evaluate_expression(callee, callee_span, symbols, ctx);
    let callee_type = match callee_result {
        InferredExpression::Known(ty) => ty,
        // The constructor target is unresolved (e.g. `new Missing(...)`). The
        // missing-name diagnostic is already reported; still evaluate the
        // arguments so their own errors surface, but do not cascade a result.
        _ => {
            for argument in arguments {
                let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            }
            return None;
        }
    };

    match callee_type {
        Type::Function(function_type) => check_function_type_call(
            &function_type,
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ),
        // A class value (static side) carries a construct signature. Check the
        // constructor arguments against it and yield the instance type.
        Type::Object(object) if object.construct_signature().is_some() => {
            let construct_signature = object
                .construct_signature()
                .expect("construct signature present")
                .clone();
            check_function_type_call(
                &construct_signature,
                callee_span,
                call_span,
                type_arguments,
                arguments,
                symbols,
                ctx,
            )
        }
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
        _ => {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2351(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    }
}

pub(crate) fn check_optional_call_like(
    callee: &surge_ts_syntax::ParsedExpression,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let callee_result = evaluate_expression(callee, callee_span, symbols, ctx);

    let callee_type = match callee_result {
        InferredExpression::Known(ty) => ty,
        _ => return None,
    };

    if callee_type == Type::Unknown {
        return None;
    }

    let base_type = surge_ts_types::remove_undefined(&callee_type);

    match base_type {
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
        Type::Function(function_type) => check_function_type_call(
            &function_type,
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        _ => {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2349(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    }
}

/// The element type a variadic rest parameter accepts per argument. A rest
/// parameter is declared as an array (`...inputs: string[]`); each supplied
/// argument is checked against the element (`string`). Non-array types (e.g. an
/// unresolved `any`) are kept as-is.
/// The span tsc anchors a too-many-arguments (TS2554) error on: the range from
/// the first excess argument through the last supplied argument. Returns `None`
/// when the relevant argument spans are unavailable so the caller can fall back
/// to the call/callee span.
fn excess_argument_span(
    arguments: &[ParsedCallArgument],
    expected: usize,
) -> Option<SyntaxTextSpan> {
    let first = arguments.get(expected)?.span?;
    let last = arguments.last().and_then(|argument| argument.span);
    Some(SyntaxTextSpan {
        start: first.start,
        end: last.map(|span| span.end).unwrap_or(first.end),
    })
}

fn rest_parameter_element_type(parameter_type: &Type) -> Type {
    match parameter_type {
        Type::Array(element) => element.as_ref().clone(),
        other => other.clone(),
    }
}

pub(crate) fn check_function_type_call(
    function_type: &FunctionType,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    _type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let required = function_type.required_parameter_count();
    let expected = function_type.parameters().len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    let too_many = !function_type.is_variadic() && actual > expected;
    if actual < required || too_many {
        let expected_count = if actual < required {
            required
        } else {
            expected
        };
        // tsc anchors a too-many-arguments error on the excess arguments (from the
        // first excess argument through the last), not on the call expression.
        let span = if too_many {
            excess_argument_span(arguments, expected)
                .or(call_span)
                .or(callee_span)
        } else {
            call_span.or(callee_span)
        };
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(expected_count, actual, ctx.file_name.clone()),
            span,
        ));
        return None;
    }

    for (i, argument) in arguments.iter().enumerate() {
        // For a variadic signature the trailing rest parameter (declared as an
        // array) matches each remaining argument against its *element* type, not
        // the array itself — `cn(...inputs: string[])` accepts `cn("a", "b")`.
        let is_rest_position = function_type.is_variadic() && expected > 0 && i >= expected - 1;
        let parameter_type: Type = if is_rest_position {
            rest_parameter_element_type(&function_type.parameters()[expected - 1])
        } else if i < expected {
            function_type.parameters()[i].clone()
        } else {
            Type::Any
        };

        let inferred_argument = evaluate_expression_with_expected_type(
            &argument.expression,
            argument.span,
            Some(&parameter_type),
            ExpectedTypeDiagnostic::ArgumentNotAssignable,
            symbols,
            ctx,
        );

        match inferred_argument {
            InferredExpression::Known(argument_type) => {
                if argument_type == Type::Unknown {
                    continue;
                }

                if !type_contains_unknown(&parameter_type)
                    && !type_contains_unknown(&argument_type)
                    && !is_assignable_to(&argument_type, &parameter_type)
                {
                    let argument_type_name = source_display_name(&argument_type, &parameter_type);
                    let parameter_type_name = parameter_type.name();
                    let diagnostic = Diagnostic::ts2345(
                        &argument_type_name,
                        &parameter_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(diagnostic, argument.span));
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. } => {
                has_unresolved_argument = true;
            }
            InferredExpression::Unknown => {}
        }
    }

    if has_unresolved_argument {
        return None;
    }

    Some(with_type_copy_reason(
        TypeCopyReason::CallResolution,
        || function_type.return_type().clone(),
    ))
}

fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Array(element) => type_contains_unknown(element),
        Type::Tuple(elements) => elements.iter().any(type_contains_unknown),
        Type::Function(function) => {
            function.parameters().iter().any(type_contains_unknown)
                || type_contains_unknown(function.return_type())
        }
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_contains_unknown(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(type_contains_unknown)
        }
        Type::Union(union) => union.types().iter().any(type_contains_unknown),
        _ => false,
    }
}
