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
    check_call_like_with_expected_type(
        callee_name,
        callee_span,
        call_span,
        type_arguments,
        arguments,
        None,
        symbols,
        ctx,
    )
}

/// `check_call_like` with the call's contextual type, so a type parameter that
/// occurs only in the return type (`const c: Ctor<MyZ> = make("x", (inst) => …)`)
/// can be inferred from it instead of degrading every callback parameter it types.
/// The callee binding degraded to `unknown`, but the arguments are still code:
/// a missing member or an unresolved name inside a callback body is reported by
/// tsc whatever the callee is. Implicit-any stays gated the way the
/// property-call path gates it: a binding that is `any` because its import
/// failed has no contextual type in tsc either, so its callbacks report their
/// parameters; a builder chain surge could not model does not.
fn evaluate_arguments_under_degraded_callee(
    callee_name: &str,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let genuine = symbols
        .get(callee_name)
        .is_some_and(|symbol| matches!(symbol.kind, crate::symbols::SymbolKind::ErrorImport));
    let saved_depth = ctx.degraded_expected_type_depth;
    if genuine {
        ctx.degraded_expected_type_depth = 0;
    } else {
        ctx.degraded_expected_type_depth += 1;
    }
    for argument in arguments {
        let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
    }
    ctx.degraded_expected_type_depth = saved_depth;
}

pub(crate) fn check_call_like_with_expected_type(
    callee_name: &str,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
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
            crate::checks::expr::unresolved_name_diagnostic(callee_name, symbols, ctx),
            callee_span,
        ));
        return None;
    };

    if symbol.ty.is_unknown() {
        evaluate_arguments_under_degraded_callee(callee_name, arguments, symbols, ctx);
        return None;
    }

    // The global `Function` interface is callable with any arguments in tsc even
    // though it declares no call signature, so it has to be answered before the
    // peel below turns it into a signature-less object.
    if surge_ts_types::is_global_function_interface(&symbol.ty) {
        for argument in arguments {
            let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
        }
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.call_expression_checking += call_start.elapsed()
        });
        return Some(Type::Any);
    }

    // A callee typed by a named declaration (`declare var Number: NumberConstructor`)
    // is a nominal reference; peel it so its call/construct signature is visible.
    let callee_ty =
        with_dts_expansion_reason(DtsExpansionReason::CallResolution, || symbol.ty.peeled());
    if callee_ty.is_unknown() {
        evaluate_arguments_under_degraded_callee(callee_name, arguments, symbols, ctx);
        return None;
    }

    // A generic call signature on an interface (`interface $Fetch { <T, R
    // extends ResponseType = "json">(…): … }`) was resolved with its type
    // parameters at their defaults; the written signature is read back so the
    // call infers them from its arguments instead.
    let generic_signature = symbol
        .function_signature
        .as_deref()
        .filter(|signature| !signature.type_parameters.is_empty());
    let written_signature = generic_signature
        .is_none()
        .then(|| interface_call_signature_info(&symbol.ty, ctx))
        .flatten();
    let outer_type_arguments = written_signature
        .as_ref()
        .map(|written| written.outer_type_arguments.as_slice())
        .unwrap_or_default();
    let result = match &callee_ty {
        Type::Function(function_type) => {
            with_type_copy_reason(TypeCopyReason::CallResolution, || {
                let function_type = instantiate_function_type(
                    function_type,
                    generic_signature
                        .or(written_signature.as_ref().map(|written| &*written.signature))
                        .or(symbol.function_signature.as_deref()),
                    outer_type_arguments,
                    type_arguments,
                    callee_span,
                    arguments,
                    expected_return_type,
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
                // A function merged with its namespace (`drizzle` + `namespace
                // drizzle { mock }`) is an object carrying the call signature;
                // its collected generic signature still drives inference.
                let call_signature = instantiate_function_type(
                    call_signature,
                    generic_signature
                        .or(written_signature.as_ref().map(|written| &*written.signature)),
                    outer_type_arguments,
                    type_arguments,
                    callee_span,
                    arguments,
                    expected_return_type,
                    symbols,
                    ctx,
                );
                check_function_type_call(
                    call_signature.as_ref(),
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

/// A written generic call signature recovered from the declaration a value is
/// typed by, paired with the bindings for the declaration's *own* type
/// parameters. tRPC's `RouterBuilder<$Root>` carries
/// `<TIn extends CreateRouterOptions>(_: TIn): BuiltRouter<$Root, …>`, whose
/// annotation names both the signature's `TIn` (inferred from the call) and the
/// interface's `$Root` (fixed by the reference), so re-resolving it needs both.
pub(crate) struct WrittenCallSignature {
    pub(crate) signature: std::sync::Arc<crate::symbols::FunctionSignatureInfo>,
    pub(crate) outer_type_arguments: Vec<(String, Type)>,
}

/// The written call signature of the declaration a value is declared as — the
/// shape whose instantiation `check_call_like` otherwise never sees, because
/// `resolve_function_type` erases a signature's type parameters to the
/// degradation sentinel and keeps only their rendering. The declaration is read
/// from its declaring file's scope; a same-file declaration falls back to the
/// current lookup.
/// A/B switch for the written-call-signature recovery, so one binary measures
/// both sides on one tree. Measurement only; the recovery is on by default.
/// The written signature of a generic function type, attached to the resolved
/// handle at resolution time so a call through a member can re-instantiate it
/// (see `resolve_function_type`). `outer_type_arguments` are the bindings the
/// body was resolved under — the enclosing interface's own parameters — so a
/// signature that names them (`filter<S extends N>`) still resolves at the call.
pub(crate) struct DeclaredMemberSignature {
    pub(crate) signature: std::sync::Arc<crate::symbols::FunctionSignatureInfo>,
    pub(crate) outer_type_arguments: Vec<(String, Type)>,
}

impl DeclaredMemberSignature {
    /// `None` when an enclosing binding is still open (the body was resolved
    /// for the uninstantiated declaration, or under a placeholder): a call
    /// re-instantiating such a signature would read `Partial<C1>` with `C1`
    /// bare and report members tsc binds through the enclosing generic.
    pub(crate) fn capture(
        function_type: &surge_ts_syntax::ParsedFunctionType,
        substitution: &crate::infer::TypeParameterSubstitution,
        ctx: &CheckerContext,
    ) -> Option<Self> {
        let own: Vec<&str> = function_type
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        let merged = crate::infer::types::merged_type_parameter_substitution(ctx, substitution);
        let mut outer_type_arguments = Vec::new();
        for (name, ty) in merged.iter() {
            if own.contains(&name.as_ref()) {
                continue;
            }
            if merged.is_placeholder(name)
                || matches!(ty, Type::Unknown | Type::TypeParameter(_))
            {
                return None;
            }
            outer_type_arguments.push((name.to_string(), ty.clone()));
        }
        let mut signature = (*crate::checks::function::function_type_signature_info(
            function_type,
            &ctx.file_name,
        ))
        .clone();
        signature.namespace_prefix = ctx
            .namespace_member_prefix_stack
            .last()
            .map(|prefix| std::sync::Arc::from(prefix.as_str()));
        Some(Self {
            signature: std::sync::Arc::new(signature),
            outer_type_arguments,
        })
    }
}

pub(crate) fn written_call_signature_recovery_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_WRITTEN_CALL_SIGNATURE").as_deref() != Ok("0"))
}

pub(crate) fn interface_call_signature_info(
    declared: &Type,
    ctx: &CheckerContext,
) -> Option<WrittenCallSignature> {
    written_call_signature_info(declared, ctx, true)
}

/// [`interface_call_signature_info`] without the generic requirement when
/// `require_generic` is false: a predicate guard reads the written signature of
/// a non-generic callable declaration too (`declare const assert: Chai.Assert`
/// declares `asserts` on its call signature, not on a function).
pub(crate) fn written_call_signature_info(
    declared: &Type,
    ctx: &CheckerContext,
    require_generic: bool,
) -> Option<WrittenCallSignature> {
    // A deferred value annotation (`let assert: typeof import("vitest")["assert"]`)
    // is a reference to the *annotation*, not to a declaration; what it resolves
    // to is the declared type this lookup wants.
    let mut declared = std::borrow::Cow::Borrowed(declared);
    for _ in 0..4 {
        let Type::Reference(reference) = declared.as_ref() else {
            break;
        };
        if !reference.id.contains("\0value-annotation\0") {
            break;
        }
        declared = std::borrow::Cow::Owned(reference.resolve());
    }
    let declared = declared.as_ref();
    if matches!(declared, Type::Reference(reference) if reference.id.contains("\0value-annotation\0")) {
        return None;
    }
    let legacy = !written_call_signature_recovery_enabled();
    let mut outer_type_arguments: Vec<Type> = Vec::new();
    let info = match declared {
        Type::Reference(reference) => {
            if legacy && !reference.arguments.is_empty() {
                return None;
            }
            outer_type_arguments = reference.arguments.to_vec();
            let id = reference.id.as_ref();
            let separator = id.rfind('\u{0}')?;
            let (declaring_file, declaration_name) = (&id[..separator], &id[separator + 1..]);
            // A global namespace member (`Chai.Assert`) is keyed by its
            // qualified name in the global table, not in its file's scope.
            ctx.module_scope_by_file
                .get(declaring_file)
                .and_then(|scope| scope.get(declaration_name))
                .or_else(|| ctx.lookup_type_declaration(declaration_name))
                .filter(|info| declaration_file_name(info) == declaring_file)
        }
        // An import binding carries the resolved object, named after its
        // interface; the declaration is whichever module scope declares that
        // name with a generic call signature.
        Type::Object(_) | Type::Function(_) => {
            let name = match declared {
                Type::Object(object) => object.alias_name.as_deref()?,
                Type::Function(function) => function.alias_name()?,
                _ => return None,
            };
            if name.contains(['<', ' ', '.']) {
                return None;
            }
            ctx.lookup_type_declaration(name).or_else(|| {
                ctx.module_scope_by_file
                    .values()
                    .filter_map(|scope| scope.get(name))
                    .find(|info| {
                        matches!(info, crate::symbols::TypeDeclarationInfo::Interface(info)
                            if info.body.call_signature.as_ref()
                                .is_some_and(|signature| !require_generic || !signature.type_parameters.is_empty()))
                    })
            })
        }
        _ => None,
    };
    let (parsed, declared_type_parameters, file_name) = match info? {
        crate::symbols::TypeDeclarationInfo::Interface(info) => {
            if legacy && !info.body.type_parameters.is_empty() {
                return None;
            }
            let parsed = info.body.call_signature.clone()?;
            (
                parsed,
                info.body.type_parameters.clone(),
                info.file_name.clone(),
            )
        }
        // `type Router = <T>(x: T) => T[]` publishes the same generic call
        // signature as the interface spelling and was refused outright, so a
        // value typed by it lost its type parameters on *every* path.
        crate::symbols::TypeDeclarationInfo::Alias(info) => {
            if legacy {
                return None;
            }
            let surge_ts_syntax::ParsedType::Function(parsed) = &info.body.ty else {
                return None;
            };
            (
                (**parsed).clone(),
                info.body.type_parameters.clone(),
                info.file_name.clone(),
            )
        }
    };
    if require_generic && parsed.type_parameters.is_empty() {
        return None;
    }
    // A declaration's own type parameters are bound by the reference that named
    // it; without them the annotation re-resolves `$Root` against nothing and
    // the instantiated return type degrades exactly as the uninstantiated one
    // did. An arity mismatch means the reference is not the one this declaration
    // describes, so nothing is bound rather than something wrong.
    let outer_type_arguments = if declared_type_parameters.len() == outer_type_arguments.len() {
        declared_type_parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .zip(outer_type_arguments)
            .collect()
    } else if declared_type_parameters.is_empty() {
        Vec::new()
    } else {
        return None;
    };
    Some(WrittenCallSignature {
        signature: crate::checks::function::function_type_signature_info(
            &parsed,
            file_name.as_ref(),
        ),
        outer_type_arguments,
    })
}

fn declaration_file_name(info: &crate::symbols::TypeDeclarationInfo) -> &str {
    match info {
        crate::symbols::TypeDeclarationInfo::Interface(info) => info.file_name.as_ref(),
        crate::symbols::TypeDeclarationInfo::Alias(info) => info.file_name.as_ref(),
    }
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

    // tsc synthesizes the union's call signature by intersecting the parameters
    // positionally, so the member that declares the most of them carries the
    // arity. A member that simply takes fewer does not make the union
    // uncallable.
    let representative = members
        .iter()
        .max_by_key(|member| member.parameters().len())
        .expect("a shared signature has at least one member");
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

/// The call signature a union member contributes. A member is callable when it
/// *is* a function type or when it peels to an object carrying a call signature
/// — the shape a callable interface takes, which is how tRPC's `AnyProcedure`
/// (a union of three `Procedure<…>` interfaces) is written. Peeling is safe
/// here: this runs only when a union is actually being called.
fn union_member_call_signature(ty: &Type) -> Option<FunctionType> {
    match ty {
        Type::Function(function_type) => Some(function_type.clone()),
        _ => match ty.peeled() {
            Type::Function(function_type) => Some(function_type),
            Type::Object(object) => object.call_signature().cloned(),
            _ => None,
        },
    }
}

/// Returns the call signatures of a union's members when every member is
/// callable and they share one Phase 1 call signature, or `None` when the union
/// is not callable under Phase 1 rules (a non-callable member, mismatched arity,
/// or parameters that are not mutually assignable). Return-type differences are
/// permitted and unified by the caller.
fn shared_signature_function_members(union: &UnionType) -> Option<Vec<FunctionType>> {
    let mut members = Vec::with_capacity(union.types().len());
    for ty in union.types() {
        members.push(union_member_call_signature(ty)?);
    }

    // Compared against the member with the most parameters, not the first: a
    // shorter member (`() => true`, a destructuring default beside
    // `EnabledFn<T>`) contributes nothing to the positions it does not declare,
    // and requiring equal arity made calling such a union a false TS2349.
    // Positions present in *both* must still agree in the strong sense — that is
    // what keeps a genuinely conflicting union uncallable.
    let longest = members
        .iter()
        .max_by_key(|member| member.parameters().len())?;
    let shares_signature = members.iter().all(|member| {
        member.is_variadic() == longest.is_variadic()
            && member
                .parameters()
                .iter()
                .zip(longest.parameters().iter())
                .all(|(member_parameter, longest_parameter)| {
                    is_assignable_to(member_parameter, longest_parameter)
                        && is_assignable_to(longest_parameter, member_parameter)
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
            // Explicit type arguments name things visible at the CALL, including
            // function locals and parameters (`new Promise<typeof opts.input>`);
            // `ctx.symbols` is the file-level table and does not hold them, so
            // resolving against it reported the local as an unresolved name.
            let explicit_args: Option<Vec<Type>> = if !type_arguments.is_empty() {
                let saved_symbols = std::mem::replace(
                    &mut ctx.symbols,
                    symbols.clone_with_reason(TypeCopyReason::CallResolution),
                );
                let resolved = type_arguments
                    .iter()
                    .map(|argument| crate::infer::map_parsed_type(argument.clone(), ctx))
                    .collect();
                ctx.symbols = saved_symbols;
                Some(resolved)
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
            let named = ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
                name: name.clone(),
                span: None,
                type_arguments,
            }));
            let saved_symbols = std::mem::replace(
                &mut ctx.symbols,
                symbols.clone_with_reason(TypeCopyReason::CallResolution),
            );
            let instance = crate::infer::map_parsed_type(named, ctx);
            ctx.symbols = saved_symbols;
            return Some(instance);
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
        // `new (Custom ?? Default)(…)`: a union whose every member is
        // constructable is constructable, and the result is the union of the
        // instance types. Arguments are checked against the first member only —
        // checking each would report the same argument diagnostics per member.
        Type::Union(union)
            if union
                .types()
                .iter()
                .all(|member| construct_signature_of(member).is_some()) =>
        {
            let signatures: Vec<surge_ts_types::FunctionType> = union
                .types()
                .iter()
                .filter_map(construct_signature_of)
                .collect();
            let mut results = Vec::with_capacity(signatures.len());
            for (index, signature) in signatures.iter().enumerate() {
                if index == 0 {
                    if let Some(result) = check_function_type_call(
                        signature,
                        callee_span,
                        call_span,
                        type_arguments,
                        arguments,
                        symbols,
                        ctx,
                    ) {
                        results.push(result);
                    }
                } else {
                    results.push(signature.return_type().clone());
                }
            }
            (!results.is_empty()).then(|| surge_ts_types::union_type(results))
        }
        // A generic class's value side is deliberately modelled as `any`
        // (`build_class_value_symbol_with_scope`), which would open the whole
        // instance: every read off `new C<Args>()` silently accepted. The class's
        // *type* side is exact, so build the instance by name the way an
        // annotation does. Only the instance is recovered here; the constructor's
        // own arity still goes unchecked, which is what the `any` value side
        // already implied.
        Type::Any => generic_class_instance_type(callee, type_arguments, arguments, symbols, ctx)
            .or(Some(Type::Any)),
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => None,
        _ => {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2351(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    }
}

/// The instance type of `new C<Args>()` where `C` is a generic class. A type
/// argument the call does not write takes the declared default, else the
/// declared constraint, else `unknown` — the order tsc resolves them in, and the
/// reason `new TRPCBuilder()` against `<TContext extends object, TMeta extends
/// object>` is `TRPCBuilder<object, object>` rather than an error.
/// Whether a written type mentions any of `names`, used to detect a type
/// parameter default or constraint written in terms of its siblings. Unhandled
/// composite forms answer `true`: the caller treats a mention as "do not
/// synthesize", so an unknown shape stays on the conservative side.
fn parsed_type_mentions_any(ty: &ParsedType, names: &[&str]) -> bool {
    match ty {
        ParsedType::Named(named) => {
            names.contains(&named.name.as_str())
                || named
                    .type_arguments
                    .iter()
                    .any(|argument| parsed_type_mentions_any(argument, names))
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            parsed_type_mentions_any(inner, names)
        }
        ParsedType::Tuple(types) | ParsedType::Union(types) | ParsedType::Intersection(types) => {
            types
                .iter()
                .any(|member| parsed_type_mentions_any(member, names))
        }
        // The `object` keyword lowers to an empty object type; a written object
        // type mentions a name only through its members.
        ParsedType::Object(object) => {
            object.call_signature.is_some()
                || object.construct_signature.is_some()
                || object
                    .string_index_type
                    .as_ref()
                    .is_some_and(|index| parsed_type_mentions_any(index, names))
                || object
                    .properties
                    .iter()
                    .any(|property| parsed_type_mentions_any(&property.ty, names))
        }
        ParsedType::String
        | ParsedType::Number
        | ParsedType::Boolean
        | ParsedType::BigInt
        | ParsedType::Symbol
        | ParsedType::Undefined
        | ParsedType::Void
        | ParsedType::Any
        | ParsedType::Unknown
        | ParsedType::UnknownKeyword
        | ParsedType::Never
        | ParsedType::StringLiteral(_)
        | ParsedType::NumberLiteral(_)
        | ParsedType::BooleanLiteral(_) => false,
        _ => true,
    }
}

pub(crate) fn generic_class_instance_type(
    callee: &ParsedExpression,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let ParsedExpression::Identifier { name, .. } = callee else {
        return None;
    };
    // A written argument names types visible at the CALL; a synthesized one is a
    // declared default or constraint and names types visible where the CLASS is
    // declared (Prisma's `ClientOptions`, socket.io's `DefaultEventMap`), which
    // the consumer's scope does not hold. The two need different scopes, so the
    // mixed case — some written, some synthesized — is left alone rather than
    // resolved under one of them and reported as an unresolved name.
    let synthesize = type_arguments.is_empty();
    let (declared, declaring_file) = {
        let Some(crate::symbols::TypeDeclarationInfo::Interface(info)) =
            ctx.lookup_type_declaration(name)
        else {
            return None;
        };
        let declared = &info.body.type_parameters;
        if declared.is_empty() {
            return None;
        }
        if !synthesize && type_arguments.len() != declared.len() {
            return None;
        }
        (
            synthesize.then(|| declared.clone()).unwrap_or_default(),
            info.file_name.clone(),
        )
    };
    // What the constructor's arguments prove, which outranks a declared default:
    // `new MutationObserver(client, { mutationFn: (text: string) => … })` is
    // `TVariables = string`, not the `void` the declaration defaults to.
    let inferred = synthesize
        .then(|| infer_generic_class_type_arguments(name, &declared, arguments, symbols, ctx))
        .flatten()
        .unwrap_or_default();
    let use_declared_defaults = synthesize && declared.iter().all(|parameter| {
        parameter.default_type.is_some()
            && (inferred.get(&parameter.name).is_none() || inferred.is_placeholder(&parameter.name))
    });
    let arguments: Vec<ParsedType> = if use_declared_defaults {
        Vec::new()
    } else if synthesize {
        let names: Vec<&str> = declared.iter().map(|p| p.name.as_str()).collect();
        let mut synthesized = Vec::with_capacity(declared.len());
        for parameter in declared.iter() {
            if let Some(inferred_type) = inferred.get(&parameter.name)
                && !inferred.is_placeholder(&parameter.name)
            {
                // Written back as the type it resolved to whenever the syntax can
                // say it, so the instance is built exactly as an explicit
                // `new C<string, …>()` is. Resolving a synthesized `Named(param)`
                // through the substitution binds the *argument* correctly but a
                // member's lazy alias reference (`subscribe(listener:
                // Listener<A, B, C>)`) re-reads its parsed arguments later, past
                // the substitution, and came out with the class's own parameters
                // unbound. A type the syntax cannot spell keeps the name route.
                synthesized.push(reify_type_argument(inferred_type).unwrap_or_else(|| {
                    ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
                        name: parameter.name.clone(),
                        span: None,
                        type_arguments: Vec::new(),
                    }))
                }));
                continue;
            }
            let fallback = parameter
                .default_type
                .clone()
                .or_else(|| parameter.constraint.clone())
                .unwrap_or(ParsedType::UnknownKeyword);
            // A default may be written in terms of its *siblings*
            // (zod's `Output = objectOutputType<T, Catchall, UnknownKeys>`).
            // Those are type parameters, not names in the declaring file, so
            // resolving the default there reports them as unresolved. Binding
            // them means resolving the list left to right under a growing
            // substitution, which this path does not carry — so a class written
            // that way keeps the permissive value side it has today.
            if parsed_type_mentions_any(&fallback, &names) {
                return None;
            }
            synthesized.push(fallback);
        }
        synthesized
    } else {
        type_arguments.to_vec()
    };
    let named = ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
        name: name.clone(),
        span: None,
        type_arguments: arguments,
    }));
    let saved_symbols = std::mem::replace(
        &mut ctx.symbols,
        symbols.clone_with_reason(TypeCopyReason::CallResolution),
    );
    let saved_file = synthesize.then(|| {
        let saved = ctx.file_name.clone();
        ctx.set_file_name(declaring_file.to_string());
        saved
    });
    let instance = crate::infer::map_parsed_type_with_substitution(named, ctx, &inferred);
    if let Some(saved) = saved_file {
        ctx.set_file_name(saved);
    }
    ctx.symbols = saved_symbols;
    (!instance.is_unknown()).then_some(instance)
}

/// The written form of a resolved type, for the cases the syntax spells
/// directly: primitives, literals, and arrays/tuples/unions of those. `None` for
/// anything nominal or structural, which has no context-free spelling here.
fn reify_type_argument(ty: &Type) -> Option<ParsedType> {
    Some(match ty {
        Type::String => ParsedType::String,
        Type::Number => ParsedType::Number,
        Type::Boolean => ParsedType::Boolean,
        Type::BigInt => ParsedType::BigInt,
        Type::Symbol => ParsedType::Symbol,
        Type::Undefined => ParsedType::Undefined,
        Type::Void => ParsedType::Void,
        Type::Any => ParsedType::Any,
        Type::Never => ParsedType::Never,
        Type::GenuineUnknown => ParsedType::UnknownKeyword,
        Type::StringLiteral(value) => ParsedType::StringLiteral(value.to_string()),
        Type::NumberLiteral(literal) => ParsedType::NumberLiteral(literal.value.to_string()),
        Type::BooleanLiteral(value) => ParsedType::BooleanLiteral(*value),
        Type::Array(element) => ParsedType::Array(std::sync::Arc::new(reify_type_argument(element)?)),
        Type::Tuple(elements) => ParsedType::Tuple(std::sync::Arc::new(
            elements
                .iter()
                .map(reify_type_argument)
                .collect::<Option<Vec<_>>>()?,
        )),
        Type::Union(union) => ParsedType::Union(std::sync::Arc::new(
            union
                .types()
                .iter()
                .map(reify_type_argument)
                .collect::<Option<Vec<_>>>()?,
        )),
        _ => return None,
    })
}

/// The class type arguments the constructor call infers, by the same machinery a
/// generic function call uses. The written constructor signature rides on the
/// class's value symbol; without one (no explicit constructor, or a class bound
/// before its signature was attached) nothing is inferred and the caller keeps
/// the declared defaults.
fn infer_generic_class_type_arguments(
    name: &str,
    declared: &[surge_ts_syntax::ParsedTypeParameter],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<crate::infer::TypeParameterSubstitution> {
    if arguments.is_empty() {
        return None;
    }
    let signature = symbols.get(name)?.function_signature.clone()?;
    if signature.type_parameters.len() != declared.len() {
        return None;
    }
    Some(infer_type_argument_substitution(
        &signature, arguments, &[], None, symbols, ctx,
    ))
}

/// Checks a call whose callee is an arbitrary expression (an IIFE, a call on a
/// call). The callee and the arguments are always evaluated so everything
/// written inside them is checked; the result is the callee's return type.
pub(crate) fn check_expression_call(
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
        InferredExpression::Known(ty) => ty.peeled(),
        _ => {
            ctx.degraded_expected_type_depth += 1;
            for argument in arguments {
                let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            }
            ctx.degraded_expected_type_depth -= 1;
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
        // A callee surge could not reduce to a signature still has one for tsc,
        // so the arguments are evaluated under a degraded expectation: their own
        // diagnostics surface, but an implicit-any report describing surge's
        // missing contextual type does not (`describe.each(cases)(name, cb)`).
        _ => {
            ctx.degraded_expected_type_depth += 1;
            for argument in arguments {
                let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            }
            ctx.degraded_expected_type_depth -= 1;
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
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => None,
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

/// The construct signature a `new` target carries: a function type is its own
/// (surge models a class value's static side as an object), an object supplies
/// its declared one, and a reference peels to whichever it resolves to.
fn construct_signature_of(ty: &Type) -> Option<surge_ts_types::FunctionType> {
    match ty.peeled() {
        Type::Function(function_type) => Some(function_type),
        Type::Object(object) => object.construct_signature().cloned(),
        _ => None,
    }
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
    let mut mismatch_reported = false;

    // A trailing parameter typed `void` (or a union containing `void`) is optional
    // at the call site — `cb()` is valid for `cb: (x: void) => void`, and a
    // `Promise<void>` executor's `resolve: (value: void | PromiseLike<void>) => void`
    // accepts `resolve()`.
    let parameters = function_type.parameters();
    let mut required = function_type.required_parameter_count();
    while required > 0 && parameter_is_void_optional(&parameters[required - 1]) {
        required -= 1;
    }

    // `f(...xs)` supplies as many arguments as the spread's type has elements,
    // which is one for a tuple of one and any number for an array. Counting the
    // spread as a single argument made `three(...tupleOfThree)` a false TS2554,
    // so a call carrying one has no statically known count and skips the check.
    let has_spread_argument = arguments.iter().any(|argument| argument.spread);
    let too_many = !function_type.is_variadic() && actual > expected;
    if !has_spread_argument && (actual < required || too_many) {
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

    // What each argument evaluated to, for overload selection afterwards. A
    // `None` type is a wildcard: a callback (typed by whichever overload is
    // picked, so it cannot pick), a degraded or unresolved argument, or a
    // spread.
    let mut argument_types: Vec<ArgumentShape> = Vec::with_capacity(arguments.len());

    for (i, argument) in arguments.iter().enumerate() {
        // A spread stands for however many arguments its type holds, so it does
        // not line up with the parameter at this position — checking it against
        // one would report the whole tuple against a single parameter. Its own
        // expression is still evaluated so errors inside it surface.
        if argument.spread {
            let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
            argument_types.push(ArgumentShape::wildcard());
            continue;
        }

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

        // tsc reports a call's first inapplicable argument and stops; later
        // arguments are still evaluated (their own diagnostics and contextual
        // typing survive), only their mismatch is withheld.
        let argument_diagnostic_span = argument.span.map(crate::context::convert_span);
        let outer_suppressed = ctx.suppressed_argument_mismatch_span;
        if mismatch_reported {
            ctx.suppressed_argument_mismatch_span = argument_diagnostic_span;
        }
        let diagnostics_before = ctx.diagnostics.len();
        let inferred_argument = evaluate_expression_with_expected_type(
            &argument.expression,
            argument.span,
            Some(&parameter_type),
            ExpectedTypeDiagnostic::ArgumentNotAssignable,
            symbols,
            ctx,
        );
        ctx.suppressed_argument_mismatch_span = outer_suppressed;
        if !mismatch_reported
            && ctx.diagnostics[diagnostics_before..].iter().any(|diagnostic| {
                matches!(
                    diagnostic.code,
                    surge_ts_diagnostics::DiagnosticCode::TypeScript(2345)
                ) && diagnostic.span.is_some()
                    && diagnostic.span == argument_diagnostic_span
            })
        {
            mismatch_reported = true;
        }

        argument_types.push(ArgumentShape {
            ty: match &inferred_argument {
                InferredExpression::Known(argument_type)
                    if !argument_is_callback(&argument.expression)
                        && !type_contains_unknown(argument_type) =>
                {
                    Some(argument_type.clone())
                }
                _ => None,
            },
            written_keys: written_object_keys(&argument.expression),
        });

        match inferred_argument {
            InferredExpression::Known(argument_type) => {
                // The sentinel and an open type parameter say nothing about
                // the source; the `unknown` keyword does — tsc rejects it for
                // every parameter that is not `unknown`/`any`.
                if matches!(argument_type, Type::Unknown | Type::TypeParameter(_))
                    || mismatch_reported
                {
                    continue;
                }
                let genuine_unknown_argument = matches!(argument_type, Type::GenuineUnknown);

                // A `never` parameter is an exhaustiveness assertion
                // (`util.assertNever(check)`): reporting it requires having
                // narrowed the argument to `never`, which surge's narrowing only
                // under-approximates, so every incompletely-narrowed residual
                // would read as a false positive.
                if !matches!(parameter_type, Type::Never)
                    && !type_contains_unknown(&parameter_type)
                    && (genuine_unknown_argument || !type_contains_unknown(&argument_type))
                    && !is_open_instantiation(&argument_type)
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
                    mismatch_reported = true;
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

    // The arguments were checked once, against the permissive fold. With their
    // types in hand, the return type is the first overload's that accepts them
    // — tsc's resolution order — and the fold's when none does, which keeps a
    // no-match call exactly where it was.
    let return_type = if mismatch_reported || has_spread_argument {
        None
    } else {
        select_overload_return_type(function_type, &argument_types)
    };
    Some(with_type_copy_reason(TypeCopyReason::CallResolution, || {
        return_type.unwrap_or_else(|| function_type.return_type().clone())
    }))
}

/// What overload selection knows about one argument.
struct ArgumentShape {
    /// The evaluated type, or `None` for a wildcard position.
    ty: Option<Type>,
    /// The property names an object-literal argument writes, spread-free. Known
    /// from the syntax alone, so it survives a degraded member type: an
    /// overload requiring a property the literal never writes is rejected even
    /// when the literal's type had to be a wildcard.
    written_keys: Option<Vec<String>>,
}

impl ArgumentShape {
    fn wildcard() -> Self {
        Self {
            ty: None,
            written_keys: None,
        }
    }
}

fn written_object_keys(expression: &ParsedExpression) -> Option<Vec<String>> {
    let ParsedExpression::ObjectLiteral { properties, .. } = expression else {
        return None;
    };
    if properties.iter().any(|property| property.is_spread) {
        return None;
    }
    Some(
        properties
            .iter()
            .map(|property| property.name.clone())
            .collect(),
    )
}

/// Whether an object-literal argument writing exactly `keys` can satisfy a
/// parameter of this type: every required property of the object it must land
/// in is written. A union is satisfied by any member; a type this cannot see
/// into is not judged.
fn written_keys_satisfy(parameter_type: &Type, keys: &[String]) -> bool {
    match parameter_type.peeled() {
        Type::Object(object) => object
            .required_properties()
            .all(|(name, _)| keys.iter().any(|key| key.as_str() == name.as_ref())),
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| written_keys_satisfy(member, keys)),
        _ => true,
    }
}

/// An argument whose type is *given* by the parameter it lands in: it was
/// contextually typed by the fold, so it cannot tell the overloads apart.
fn argument_is_callback(expression: &ParsedExpression) -> bool {
    matches!(expression, ParsedExpression::ArrowFunction(_))
}

/// The return type of the first overload the evaluated arguments satisfy, or
/// `None` when the signature carries no group or no member accepts them.
fn select_overload_return_type(
    function_type: &FunctionType,
    argument_types: &[ArgumentShape],
) -> Option<Type> {
    let overloads = function_type.overloads()?;
    crate::program::record_overload_selection_attempt();
    let picked = overloads
        .iter()
        .find(|candidate| signature_accepts_argument_types(candidate, argument_types))?;
    // A member whose return still names a type parameter, or stands at the
    // sentinel, was folded before instantiation and never re-resolved for this
    // call — a generic group reached without its written signature. Its return
    // is the declaration's, not the call's; the fold's answer (which widened
    // such disagreements to `any`) is the honest one. zod's
    // `registry.get(schema)?.id` came back as an unbound `$replace<Meta, S>`
    // without this.
    if type_contains_unknown(picked.return_type()) {
        return None;
    }
    crate::program::record_overload_selection_pick();
    Some(picked.return_type().clone())
}

fn signature_accepts_argument_types(
    signature: &FunctionType,
    argument_types: &[ArgumentShape],
) -> bool {
    let parameters = signature.parameters();
    let expected = parameters.len();
    let actual = argument_types.len();
    let mut required = signature.required_parameter_count();
    while required > 0 && parameter_is_void_optional(&parameters[required - 1]) {
        required -= 1;
    }
    if actual < required || (!signature.is_variadic() && actual > expected) {
        return false;
    }

    argument_types.iter().enumerate().all(|(i, argument)| {
        let is_rest_position = signature.is_variadic() && expected > 0 && i >= expected - 1;
        let parameter_type = if is_rest_position {
            rest_parameter_element_type(&parameters[expected - 1], i - (expected - 1))
        } else if i < expected {
            let declared = parameters[i].clone();
            if i >= signature.required_parameter_count() {
                union_type(vec![declared, Type::Undefined])
            } else {
                declared
            }
        } else {
            return true;
        };
        if let Some(keys) = argument.written_keys.as_deref()
            && !written_keys_satisfy(&parameter_type, keys)
        {
            return false;
        }
        let Some(argument_type) = argument.ty.as_ref() else {
            return true;
        };
        // A parameter standing at the degradation sentinel proves nothing about
        // this position; committing to such an overload would hand its (equally
        // degraded) return to every consumer.
        !type_contains_unknown(&parameter_type)
            && is_assignable_to(argument_type, &parameter_type)
            && !weak_type_rejects(argument_type, &parameter_type)
    })
}

/// tsc's weak-type rule, applied to selection only: an object type whose
/// properties are all optional accepts nothing that shares no property with it.
/// surge's assignability is lenient there — `'utf8'` passes against
/// `{ encoding?: null; flag?: string } | null` — and a lenient match commits to
/// the wrong overload: `fs.readFileSync(path, 'utf8')` picked the `Buffer`
/// overload on both zod and trpc. Rejects when every parameter member that
/// accepted the argument is a weak object the argument shares nothing with.
fn weak_type_rejects(argument_type: &Type, parameter_type: &Type) -> bool {
    let parameter_type = parameter_type.peeled();
    let members: Vec<Type> = match &parameter_type {
        Type::Union(union) => union.types().to_vec(),
        other => vec![other.clone()],
    };
    let mut accepting = 0usize;
    let mut weakly_rejected = 0usize;
    for member in &members {
        if !is_assignable_to(argument_type, member) {
            continue;
        }
        accepting += 1;
        if let Type::Object(object) = member.peeled()
            && is_weak_object(&object)
            && !shares_a_property(argument_type, &object)
        {
            weakly_rejected += 1;
        }
    }
    accepting > 0 && accepting == weakly_rejected
}

fn is_weak_object(object: &surge_ts_types::ObjectType) -> bool {
    !object.properties.is_empty()
        && object.required_properties().next().is_none()
        && !object.declares_string_index_access()
        && object.call_signature().is_none()
        && object.construct_signature().is_none()
}

/// Whether `argument_type` is an object carrying at least one of `target`'s
/// property names, or an empty object (which tsc lets through). Primitives,
/// arrays and functions share nothing.
fn shares_a_property(argument_type: &Type, target: &surge_ts_types::ObjectType) -> bool {
    match argument_type.peeled() {
        Type::Any => true,
        Type::Object(object) => {
            object.properties.is_empty()
                || object
                    .properties
                    .keys()
                    .any(|name| target.contains_property(name))
        }
        _ => false,
    }
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
        overloaded: false,
        type_parameters: parsed.type_parameters.clone(),
        parameter_types: parsed
            .parameters
            .iter()
            .map(|parameter| Some(parameter.ty.clone()))
            .collect(),
        parameter_names: parsed
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        rest: parsed.parameters.last().is_some_and(|parameter| parameter.rest),
        return_type: Some((*parsed.return_type).clone()),
        declaring_file: None,
        namespace_prefix: None,
        predicate_overload: None,
        overload_alternatives: Vec::new(),
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

/// A type argument surge could not resolve, as opposed to one that merely
/// mentions a type parameter. A *generic* function value
/// (`vi.fn<typeof fetchData>(fetchData)`) carries its own parameters as
/// placeholders; refusing it as a candidate left the call uninstantiated and
/// every use of the result degraded.
pub(crate) fn type_argument_is_unresolved(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Array(element) => type_argument_is_unresolved(element),
        Type::Tuple(elements) => elements.iter().any(type_argument_is_unresolved),
        Type::Function(function) => {
            function
                .parameters()
                .iter()
                .any(type_argument_is_unresolved)
                || type_argument_is_unresolved(function.return_type())
        }
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_argument_is_unresolved(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(type_argument_is_unresolved)
        }
        Type::Union(union) => union.types().iter().any(type_argument_is_unresolved),
        _ => false,
    }
}

/// `Mock<T>` with `T` bare, `FetchQueryOptions<TQueryFnData>` inside a generic
/// body: an instantiation over an open argument is not settled enough to reject
/// on either side of a check. Only the type itself is asked — a generic
/// function *value* whose parameter names its own `T` is still a concrete
/// value (`const n: number = Controller` is a real mismatch).
pub(crate) fn is_open_instantiation(ty: &Type) -> bool {
    fn argument_is_open(ty: &Type) -> bool {
        match ty {
            Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => true,
            Type::Reference(reference) => reference.arguments.iter().any(argument_is_open),
            _ => false,
        }
    }
    matches!(ty, Type::Reference(reference) if reference.arguments.iter().any(argument_is_open))
}

pub(crate) fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => true,
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
