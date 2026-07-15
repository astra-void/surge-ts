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
use crate::program::{
    DtsExpansionReason, record_call_resolution, record_program_timing, with_dts_expansion_reason,
};
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
    // A call may target a module-scope binding declared later in the file when it
    // sits inside a function body (the body runs after the module is evaluated),
    // so fall back to the module value table before reporting an unresolved name.
    let fallback_symbol = if symbols.get(callee_name).is_none() {
        ctx.module_value_fallback
            .as_ref()
            .and_then(|fallback| fallback.get(callee_name))
            .cloned()
    } else {
        None
    };
    let Some(symbol) = symbols.get(callee_name).or(fallback_symbol.as_ref()) else {
        if emit_type_only_as_value_diagnostic(callee_name, callee_span, ctx) {
            return None;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(callee_name, ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    if symbol.ty.is_unknown() {
        return None;
    }

    // A callee typed by a named declaration (`declare var Number: NumberConstructor`)
    // is a nominal reference; peel it so its call/construct signature is visible.
    let callee_ty =
        with_dts_expansion_reason(DtsExpansionReason::CallResolution, || symbol.ty.peeled());
    if callee_ty.is_unknown() {
        return None;
    }

    let result = match &callee_ty {
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
        // An `any`-typed callee is callable and yields `any`; still evaluate the
        // arguments so their own errors surface. (`new`-position handles this in
        // `check_new_like`; this is the symmetric call-position arm.)
        Type::Any => {
            for argument in arguments {
                let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            }
            Some(Type::Any)
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
    if union.types().iter().any(|ty| ty.is_unknown()) {
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
    expected_type: Option<&Type>,
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
        let physical_interface_arity = match ctx.lookup_type_declaration(name) {
            Some(crate::symbols::TypeDeclarationInfo::Interface(info)) => {
                if crate::default_lib::is_physical_default_lib_file_name(&info.file_name) {
                    Some(info.body.type_parameters.len())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(arity) = physical_interface_arity {
            // The constructor value (e.g. `Promise` typed `PromiseConstructor`)
            // carries the construct signature. Keep its full type so we can re-read
            // the declaring constructor-interface's generic construct signature.
            let constructor_type = match evaluate_expression(callee, callee_span, symbols, ctx) {
                InferredExpression::Known(ty) => Some(ty),
                _ => None,
            };
            let construct_signature = constructor_type.as_ref().and_then(|ty| match ty.peeled() {
                Type::Object(object) => object.construct_signature().cloned(),
                _ => None,
            });

            // Resolve the constructor's type arguments: explicit `new Promise<void>()`
            // first, else infer from a contextual expected type that is a reference
            // to this same interface (`const p: Promise<void> = new Promise(...)`).
            // These pin the generic construct signature so the executor's callback
            // parameters are typed (`resolve: (value: T | PromiseLike<T>) => void`
            // becomes `... void | PromiseLike<void> ...`, making `resolve()` valid).
            let explicit_args: Option<Vec<Type>> = if !type_arguments.is_empty() {
                Some(
                    type_arguments
                        .iter()
                        .map(|argument| crate::infer::map_parsed_type(argument.clone(), ctx))
                        .collect(),
                )
            } else {
                None
            };
            let contextual_instance = if explicit_args.is_none() && arity > 0 {
                expected_type
                    .and_then(|expected| contextual_instance_reference(name, arity, expected))
            } else {
                None
            };
            let substitution_args = explicit_args.clone().or_else(|| {
                contextual_instance
                    .as_ref()
                    .map(|(_, arguments)| arguments.clone())
            });

            // Check the arguments against the construct signature so callback
            // parameters get contextual types instead of collapsing to implicit
            // `any`. When no construct signature is reachable, evaluate bare.
            if let Some(construct_signature) = construct_signature {
                let substituted = substitution_args.as_ref().and_then(|args| {
                    constructor_type.as_ref().and_then(|ctor_ty| {
                        substituted_construct_signature(ctor_ty, &construct_signature, args, ctx)
                    })
                });
                let effective_signature = substituted.as_ref().unwrap_or(&construct_signature);
                with_type_copy_reason(TypeCopyReason::CallResolution, || {
                    check_function_type_call(
                        effective_signature,
                        callee_span,
                        call_span,
                        type_arguments,
                        arguments,
                        symbols,
                        ctx,
                    )
                });
            } else {
                for argument in arguments {
                    let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
                }
            }

            // The contextual expected reference IS the instance type when present.
            if let Some((instance, _)) = contextual_instance {
                return Some(instance);
            }

            // Build the instance interface type (`Map<K, V>`) so lib methods carry
            // meaningful types. With explicit type arguments use them; otherwise
            // default each missing argument to `any` — a bare `Set<>` would trip
            // the generic-arity TS2314, while `Set<any>` stays assignable to
            // whatever the use site expects.
            let type_arguments = if type_arguments.is_empty() && arity > 0 {
                vec![ParsedType::Any; arity]
            } else {
                type_arguments.to_vec()
            };
            let named = ParsedType::Named(ParsedNamedType {
                name: name.clone(),
                span: None,
                type_arguments,
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
        // A constructor value is often a nominal reference (`declare const P:
        // PromiseConstructor`); peel it so its construct signature is visible.
        InferredExpression::Known(ty) => ty.peeled(),
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
        Type::Unknown | Type::GenuineUnknown => None,
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

    if callee_type.is_unknown() {
        return None;
    }

    let base_type = surge_ts_types::remove_undefined(&callee_type);

    match base_type {
        Type::Any => Some(Type::Any),
        Type::Unknown | Type::GenuineUnknown => None,
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

fn rest_parameter_element_type(parameter_type: &Type, rest_offset: usize) -> Type {
    match parameter_type {
        Type::Array(element) => element.as_ref().clone(),
        // A tuple-typed rest parameter (`...args: [name: string]`) accepts each
        // argument at its tuple position; an overload folded into a union of
        // tuples (`...args: [string] | [RequestCookie]`, next's cookie store)
        // accepts the union of the per-position elements. Without these arms the
        // whole tuple/union was compared against each single argument — a false
        // TS2345 on every call.
        Type::Tuple(elements) => elements
            .get(rest_offset)
            .or_else(|| elements.last())
            .cloned()
            .unwrap_or(Type::Any),
        Type::Union(union) => union_type(
            union
                .types()
                .iter()
                .map(|member| rest_parameter_element_type(member, rest_offset))
                .collect(),
        ),
        Type::Reference(_) => rest_parameter_element_type(&parameter_type.peeled(), rest_offset),
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
    let expected = function_type.parameters().len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    // A trailing parameter typed `void` (or a union containing `void`) is optional
    // at the call site — `cb()` is valid for `cb: (x: void) => void`, and a
    // `Promise<void>` executor's `resolve: (value: void | PromiseLike<void>) => void`
    // accepts `resolve()`.
    let parameters = function_type.parameters();
    let mut required = function_type.required_parameter_count();
    while required > 0 && parameter_is_void_optional(&parameters[required - 1]) {
        required -= 1;
    }

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
            rest_parameter_element_type(
                &function_type.parameters()[expected - 1],
                i - (expected - 1),
            )
        } else if i < expected {
            let declared = function_type.parameters()[i].clone();
            // An optional parameter (`x?: T`, declared past the required count)
            // accepts `undefined` at the call site — passing `T | undefined` to it
            // is valid, matching tsc. Widen so a provided argument is checked
            // against `T | undefined` rather than the bare `T`.
            if i >= function_type.required_parameter_count()
                && !is_assignable_to(&Type::Undefined, &declared)
            {
                union_type(vec![declared, Type::Undefined])
            } else {
                declared
            }
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
                if argument_type.is_unknown() {
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

/// When `expected` is a nominal reference to the interface `name` with `arity`
/// type arguments (e.g. the contextual `Promise<void>` for an un-annotated
/// `new Promise(...)`), returns that reference (the instance type) together with
/// its resolved type arguments.
fn contextual_instance_reference(
    name: &str,
    arity: usize,
    expected: &Type,
) -> Option<(Type, Vec<Type>)> {
    let Type::Reference(reference) = expected else {
        return None;
    };
    let display = reference.display.as_ref();
    let base = display.split('<').next().unwrap_or(display);
    if base != name || reference.arguments.len() != arity {
        return None;
    }
    Some((expected.clone(), reference.arguments.to_vec()))
}

/// Re-resolves a generic construct signature with `type_arguments` substituted
/// for its per-signature type parameters, by re-reading the declaring
/// constructor interface's parsed construct signature (`PromiseConstructor`'s
/// `new <T>(executor): Promise<T>`). Returns `None` when the interface has no
/// matching generic construct signature, so callers fall back to the
/// (un-substituted) resolved signature.
fn substituted_construct_signature(
    constructor_type: &Type,
    base_signature: &FunctionType,
    type_arguments: &[Type],
    ctx: &mut CheckerContext,
) -> Option<FunctionType> {
    let constructor_name = constructor_type.name();
    let parsed = match ctx.lookup_type_declaration(&constructor_name) {
        Some(crate::symbols::TypeDeclarationInfo::Interface(info)) => info
            .body
            .construct_signatures
            .iter()
            .find(|signature| {
                !signature.type_parameters.is_empty()
                    && signature.type_parameters.len() == type_arguments.len()
                    && signature.parameters.len() == base_signature.parameters().len()
            })
            .cloned(),
        _ => None,
    }?;

    let signature_info = crate::symbols::FunctionSignatureInfo {
        type_parameters: parsed.type_parameters.clone(),
        parameter_types: parsed
            .parameters
            .iter()
            .map(|parameter| Some(parameter.ty.clone()))
            .collect(),
        return_type: Some((*parsed.return_type).clone()),
        declaring_file: None,
    };
    let mut substitution = crate::infer::TypeParameterSubstitution::new();
    for (type_parameter, argument) in parsed.type_parameters.iter().zip(type_arguments.iter()) {
        substitution.insert(type_parameter.name.clone(), argument.clone());
    }

    Some(
        instantiate_function_type_with_substitution(
            base_signature,
            &signature_info,
            &substitution,
            false,
            ctx,
        )
        .into_owned(),
    )
}

/// Whether a parameter of this type may be omitted at a call site. tsc treats a
/// parameter typed `void` — or a union with a `void` member (e.g. a `Promise<void>`
/// executor's `resolve: (value: void | PromiseLike<void>) => void`) — as
/// optional, but not `undefined`.
fn parameter_is_void_optional(ty: &Type) -> bool {
    match ty {
        Type::Void => true,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Void)),
        _ => false,
    }
}

fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown | Type::GenuineUnknown => true,
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
