use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedFunctionDeclaration, ParsedTypeParameter,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use super::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{FunctionFlowState, analyze_function_body_flow, collect_function_flow_facts};
use crate::infer::InferredExpression;
use crate::metrics::alloc_function_type;
use crate::program::record_program_timing;
use crate::symbols::{ScopeStack, SymbolTable};

mod body;
mod body_statements;
mod narrowing;
mod signature;

pub(crate) use body::*;
pub(crate) use body_statements::*;
pub(crate) use narrowing::*;
pub(crate) use signature::*;
/// tsc's `getReturnTypeFromBody`, restricted to a body whose `return`s all sit
/// at the top level.
///
/// A function declaration's signature is collected before any body is checked,
/// so an unannotated return type stayed the degradation sentinel — and every
/// type parameter a caller would infer *through* that return died with it.
/// Inferring by checking the body here would walk it twice, so this reads only
/// the body's own top-level statements: the bindings a `return` reads, and the
/// `return`s themselves. Any statement that could hide a `return` gives up and
/// keeps the sentinel, so a conditional body is never guessed at.
///
/// A returned `any` is refused along with the sentinel. tsc never produces
/// `any` from a body that returns a typed value, so one here is surge's own gap
/// — and publishing it is strictly worse than the sentinel: `any` is absorbing,
/// so it silences every downstream check (`noUncheckedIndexedAccess` included)
/// instead of merely staying unknown.
fn inferred_declaration_return_type(
    function: &ParsedFunctionDeclaration,
    function_type: &FunctionType,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    use surge_ts_syntax::ParsedFunctionBodyStatement as BodyStatement;

    if function.return_type.is_some()
        || function.is_declare
        || !function.has_body
        || function.is_generator
        || function.body.is_empty()
    {
        return None;
    }

    let mut scope = ctx
        .symbols
        .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    for (parameter, parameter_type) in function
        .parameters
        .iter()
        .zip(function_type.parameters().iter())
    {
        if let Some(name) = signature::written_binding_names(std::slice::from_ref(parameter))
            .into_iter()
            .flatten()
            .next()
        {
            scope.insert(
                name,
                crate::symbols::SymbolInfo {
                    ty: parameter_type.clone(),
                    kind: crate::symbols::SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
    }

    let usable_type = |ty: &Type| !ty.is_unknown() && !matches!(ty, Type::Any);
    let diagnostics_before = ctx.diagnostics().len();
    let mut returned: Vec<Type> = Vec::new();
    let mut usable = true;
    for statement in &function.body {
        match statement {
            BodyStatement::VariableDeclaration(variable) => {
                if variable.declared_type.is_none()
                    && let Some(initializer) = variable.initializer.as_ref()
                    && let crate::infer::InferredExpression::Known(ty) =
                        crate::infer::infer_expression(initializer, &scope, ctx)
                {
                    scope.insert(
                        variable.name.clone(),
                        crate::symbols::SymbolInfo {
                            ty,
                            kind: crate::symbols::SymbolKind::Const,
                            function_signature: None,
                        },
                    );
                }
            }
            BodyStatement::Return(statement) => match statement.expression.as_ref() {
                Some(expression) => {
                    match crate::infer::infer_expression(expression, &scope, ctx) {
                        crate::infer::InferredExpression::Known(ty) if usable_type(&ty) => {
                            returned.push(ty)
                        }
                        _ => usable = false,
                    }
                }
                None => returned.push(Type::Undefined),
            },
            BodyStatement::Throw(_)
            | BodyStatement::Assignment(_)
            | BodyStatement::ThisPropertyAssignment(_)
            | BodyStatement::MemberAssignment(_)
            | BodyStatement::Expression(_)
            | BodyStatement::Function(_)
            | BodyStatement::TypeAlias(_)
            | BodyStatement::Continue
            | BodyStatement::Break => {}
            _ => usable = false,
        }
        if !usable {
            break;
        }
    }
    ctx.truncate_diagnostics_releasing_utility_keys(diagnostics_before);

    // tsc widens the fresh literals a returned expression carries
    // (`getReturnTypeFromBody` runs the result through the widening machinery),
    // so `return { importName: "trpc" }` is `{ importName: string }`. Freezing
    // the literal instead publishes a type far narrower than the declaration's,
    // and every consumer compares against the wrong one.
    let inferred = (usable && !returned.is_empty())
        .then(|| crate::checks::expr::widen_type(&surge_ts_types::union_type(returned)))
        .filter(usable_type)?;
    Some(inferred)
}

/// `SURGE_INFER_DECLARATION_RETURN_TYPES=1`: infer an unannotated function
/// declaration's return type from its body (see
/// [`inferred_declaration_return_type`]). Any other non-empty value is a file
/// substring filter, so the effect can be bisected across the corpus without a
/// rebuild.
fn infer_declaration_return_types(file_name: &str) -> bool {
    static SETTING: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    match SETTING
        .get_or_init(|| std::env::var("SURGE_INFER_DECLARATION_RETURN_TYPES").ok())
        .as_deref()
    {
        None | Some("") => false,
        Some("1") => true,
        Some(filter) => file_name.contains(filter),
    }
}

pub(crate) fn collect_function_declaration_signature(
    function: &ParsedFunctionDeclaration,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
    allow_lazy_dependency_signature: bool,
) -> FunctionType {
    let temp_symbols = std::mem::take(symbols);
    ctx.set_symbols(temp_symbols);

    // Establish the function's own type-parameter scope (with constraints) while
    // mapping the signature, so a constrained parameter such as `K extends keyof T`
    // is visible when its body resolves a `T[K]` indexed access (otherwise a false
    // TS2536, e.g. the lib `addEventListener<K extends keyof WindowEventMap>(…:
    // WindowEventMap[K])`).
    //
    // Scoped to *generic* `declare` functions only. For a `declare` function this
    // collected signature is the authoritative resolution (no body is checked
    // afterwards), so it must resolve its constrained indexed accesses here. A
    // non-`declare` function is re-checked authoritatively by
    // `check_function_declaration` under its own scope; resolving its (often
    // cross-module) signature concretely *here* instead changed how generic
    // instantiations were collected and surfaced assignability false positives, so
    // its pre-pass signature is left as-is. The empty-scope case is also skipped: a
    // pushed empty scope makes `type_parameter_scopes` non-empty and flips the
    // `concrete_instantiation` short-circuit.
    crate::program::record_program_counter(|c| c.function_signatures_indexed_count += 1);
    let map_signature = |ctx: &mut CheckerContext| {
        static LAZY_DEPENDENCY_SIGNATURES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let lazy_dependency_signatures = *LAZY_DEPENDENCY_SIGNATURES
            .get_or_init(|| std::env::var_os("SURGE_EAGER_DEPENDENCY_SIGNATURES").is_none());
        // Deferral stays scoped to installed-package declarations. Extending it
        // to the physical `lib.*.d.ts` set looks attractive (largest
        // declaration surface, small slice of it used) and is diagnostically
        // free, but it is a measured loss: building lazy references for the
        // whole lib surface costs more than the deferred mapping saves.
        // See docs/perf/LOADER-PARSE-HANDOFF.md § Not taken.
        if ctx.current_file_kind == crate::context::FileKind::DependencyDeclaration
            && allow_lazy_dependency_signature
            && lazy_dependency_signatures
        {
            map_lazy_dependency_function_signature(function, ctx)
        } else {
            map_function_signature(
                &function.parameters,
                function.return_type.as_ref(),
                &function.type_parameters,
                None,
                ctx,
            )
        }
    };
    let function_type = if function.is_declare && !function.type_parameters.is_empty() {
        with_type_parameter_scope(&function.type_parameters, ctx, map_signature)
    } else {
        map_signature(ctx)
    };

    let function_type = match infer_declaration_return_types(&ctx.file_name)
        .then(|| inferred_declaration_return_type(function, &function_type, ctx))
        .flatten()
    {
        Some(return_type) => FunctionType::new(
            function_type.parameters().to_vec(),
            return_type,
            function_type.is_variadic(),
            function_type.required_parameter_count(),
        )
        .with_parameter_names(signature::written_binding_names(&function.parameters)),
        None => function_type,
    };

    *symbols = std::mem::take(&mut ctx.symbols);

    let duplicate = register_function_signature(
        function.name.clone(),
        with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || function_type.clone()),
        Some(function_declaration_signature_info(
            function,
            &function_type,
            symbols,
            &ctx.file_name,
        )),
        symbols,
        false,
        function.has_body,
    );

    if duplicate {
        let diagnostic = Diagnostic::ts2393(ctx.file_name.clone());
        let diagnostic = match function.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);

        if let Some(first_span) = symbols.take_declaration_span(&function.name) {
            ctx.push(Diagnostic::ts2393(ctx.file_name.clone()).with_span(convert_span(first_span)));
        }
    } else if let Some(span) = function.name_span {
        symbols.record_declaration_span(&function.name, span);
    }

    function_type
}

pub(crate) fn check_function_declaration(
    function: ParsedFunctionDeclaration,
    ctx: &mut CheckerContext,
) {
    let start = Instant::now();
    let ParsedFunctionDeclaration {
        has_this_parameter,
        this_parameter_type,
        is_declare,
        name,
        name_span,
        type_parameters,
        parameters,
        return_type,
        return_type_span,
        body,
        has_body,
        body_reads,
        is_generator,
        is_async,
        ..
    } = function;

    with_type_parameter_scope(&type_parameters, ctx, |ctx| {
        let signature_info = function_signature_info(
            &type_parameters,
            &parameters,
            return_type.as_ref(),
            &ctx.file_name,
        );
        let function_type = map_function_signature(
            &parameters,
            return_type.as_ref(),
            &type_parameters,
            None,
            ctx,
        );

        // The first registration for this name replaces what the collection
        // pre-pass installed — the check pass resolves the signature under the
        // right scope, so its answer is the authoritative one. A *later*
        // declaration of the same name is an overload, and replacing again would
        // leave only the last one: `declare function f(v: number): string`
        // followed by `declare function f(v: number, r: number): string` made
        // `f(1)` a false TS2554.
        let first_registration = ctx
            .checked_function_declaration_names
            .insert(std::sync::Arc::from(name.as_str()));
        let duplicate = {
            let symbols = &mut ctx.symbols;
            register_function_signature(
                name.clone(),
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || function_type.clone()),
                Some(signature_info.clone()),
                symbols,
                first_registration,
                has_body,
            )
        };

        if duplicate {
            let diagnostic = Diagnostic::ts2393(ctx.file_name.clone());
            let diagnostic = match name_span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };

            ctx.push(diagnostic);

            if let Some(first_span) = ctx.symbols.take_declaration_span(&name) {
                ctx.push(
                    Diagnostic::ts2393(ctx.file_name.clone()).with_span(convert_span(first_span)),
                );
            }
        } else if let Some(span) = name_span {
            ctx.symbols.record_declaration_span(&name, span);
        }

        // A bodyless `function` declaration is an ambient declaration or an
        // overload signature: there is no body to check, and the implementation
        // (or none, for ambient) carries the real body. Running body checks here
        // would falsely flag the signature (e.g. TS2355 for a non-void return type
        // with no `return`), which tsc never does for an overload signature.
        if is_declare || !has_body {
            return;
        }

        check_function_body_with_signature(
            name,
            parameters,
            body,
            &function_type,
            &type_parameters,
            Some(signature_info),
            return_type.is_some(),
            return_type_span.or(name_span),
            has_body.then(|| body_reads.as_slice()),
            is_generator,
            is_async,
            has_this_parameter,
            this_parameter_type,
            ctx,
        );
    });
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.function_declaration_checking += start.elapsed()
    });
}

pub(crate) fn check_function_declaration_body(
    function: ParsedFunctionDeclaration,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let start = Instant::now();
    let ParsedFunctionDeclaration {
        has_this_parameter,
        this_parameter_type,
        is_declare,
        name,
        name_span,
        parameters,
        return_type,
        return_type_span,
        body,
        has_body,
        body_reads,
        is_generator,
        is_async,
        ..
    } = function;

    // A bodyless declaration is ambient or an overload signature; checking the
    // absent body would report TS2355 on a non-void return type that the
    // implementation below actually satisfies. Mirrors the guard in
    // `check_function_declaration`.
    if is_declare || !has_body {
        return;
    }

    let signature_info = function_signature_info(
        type_parameters,
        &parameters,
        return_type.as_ref(),
        &ctx.file_name,
    );
    check_function_body_with_signature(
        name,
        parameters,
        body,
        function_type,
        type_parameters,
        Some(signature_info),
        return_type.is_some(),
        return_type_span.or(name_span),
        has_body.then(|| body_reads.as_slice()),
        is_generator,
        is_async,
        has_this_parameter,
        this_parameter_type,
        ctx,
    );
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.function_declaration_checking += start.elapsed()
    });
}

/// The contextual type a written parameter takes at each position of
/// `expected_type`. A rest parameter is not one positional slot: a tuple-typed
/// one (`(...args: [value: number, message?: string]) => void`, the shape TS
/// variadic-tuple reconstruction produces) declares those elements positionally,
/// and an array-typed one supplies its element type at every trailing position.
/// Reading `parameters()` by index instead leaves every position past the rest
/// slot uncontextualized, which is a false TS7006/TS7031 on the callback.
/// The tuple expansion mirrors `expanded_signature` in the assignability
/// relation, so a callback typed here still compares against its slot.
/// The contextual type of a written rest parameter at `position`: tsc's
/// `getRestTypeAtPosition` (relater.go:1829-1858). It is the *collection* the
/// rest binds — the context's own rest type when the positions line up, an array
/// of its element past that, and otherwise a tuple of the remaining positions —
/// never the element type the positional expansion hands every other slot.
/// `(...a) => …` against `(...args: string[]) => void` typed `a` as `string`.
///
/// `None` where tsc's answer needs an optional tuple element, which surge's
/// tuple types cannot say; those positions keep the positional expansion.
fn contextual_rest_parameter_type(expected_type: &FunctionType, position: usize) -> Option<Type> {
    let parameters = expected_type.parameters();
    let count = parameters.len();
    // Surge stores a rest slot either as the written array or already unwrapped
    // to its element (see `contextual_parameter_types`); tsc's `restType` is the
    // collection, and its element is what an index by `number` reads.
    let rest_and_element = expected_type.is_variadic().then(|| {
        let stored = match &parameters[count - 1] {
            Type::Reference(reference) => reference.resolve().peeled(),
            other => other.clone(),
        };
        match stored {
            Type::Array(element) => (Type::Array(element.clone()), *element),
            Type::Tuple(elements) => {
                let element = surge_ts_types::union_type(elements.clone());
                (Type::Tuple(elements), element)
            }
            Type::OpenTuple(open) => {
                let element = open.element_union();
                (Type::OpenTuple(open), element)
            }
            element => (Type::Array(Box::new(element.clone())), element),
        }
    });

    if let Some((rest, element)) = &rest_and_element
        && position + 1 >= count
    {
        return Some(if position + 1 == count {
            rest.clone()
        } else {
            Type::Array(Box::new(element.clone()))
        });
    }

    let fixed_end = if rest_and_element.is_some() { count - 1 } else { count };
    if position >= fixed_end {
        return Some(Type::Tuple(Vec::new()));
    }
    // surge's tuples have no optional elements, so an optional parameter
    // contributes its `T | undefined` slot.
    let required = expected_type.required_parameter_count();
    let leading = parameters[position..fixed_end]
        .iter()
        .enumerate()
        .map(|(offset, parameter)| {
            if position + offset >= required
                && !surge_ts_types::is_assignable_to(&Type::Undefined, parameter)
            {
                surge_ts_types::union_type(vec![parameter.clone(), Type::Undefined])
            } else {
                parameter.clone()
            }
        })
        .collect::<Vec<_>>();
    Some(match rest_and_element {
        None => Type::Tuple(leading),
        Some((Type::Tuple(elements), _)) => Type::Tuple(leading.into_iter().chain(elements).collect()),
        Some((Type::OpenTuple(open), _)) => Type::OpenTuple(surge_ts_types::OpenTupleType {
            leading: leading.into_iter().chain(open.leading).collect(),
            rest: open.rest,
            trailing: open.trailing,
        }),
        Some((_, element)) => Type::OpenTuple(surge_ts_types::OpenTupleType {
            leading,
            rest: Box::new(element),
            trailing: Vec::new(),
        }),
    })
}

fn without_undefined(ty: Type) -> Type {
    match &ty {
        Type::Union(union) if union.types().contains(&Type::Undefined) => surge_ts_types::union_type(
            union
                .types()
                .iter()
                .filter(|member| !matches!(member, Type::Undefined))
                .cloned()
                .collect(),
        ),
        _ => ty,
    }
}

fn contextual_parameter_types(expected_type: &FunctionType, parameter_count: usize) -> Vec<Type> {
    // tsc's `getTypeOfParameter`: an optional parameter of the contextual
    // signature is `T | undefined`, and that is the type an unannotated
    // callback parameter takes from it — the caller may well omit it.
    let required = expected_type.required_parameter_count();
    let with_optionality = |index: usize, parameter: &Type| {
        if index >= required
            && surge_ts_types::strict_null_checks()
            && !parameter.is_unknown()
            && !matches!(parameter, Type::Any)
        {
            surge_ts_types::union_type(vec![parameter.clone(), Type::Undefined])
        } else {
            parameter.clone()
        }
    };
    let parameters = expected_type.parameters();
    if !expected_type.is_variadic() {
        return parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| with_optionality(index, parameter))
            .collect();
    }

    let Some((rest, leading)) = parameters.split_last() else {
        return Vec::new();
    };
    let leading: Vec<Type> = leading
        .iter()
        .enumerate()
        .map(|(index, parameter)| with_optionality(index, parameter))
        .collect();
    let leading = leading.as_slice();
    let peeled;
    let rest = match rest {
        Type::Reference(reference) => {
            peeled = reference.resolve().peeled();
            &peeled
        }
        other => other,
    };

    let mut expanded = leading.to_vec();
    match rest {
        Type::Tuple(elements) => expanded.extend(elements.iter().cloned()),
        // The resolver already unwraps an array rest annotation to its element
        // type, but a signature mapped from source keeps the array; accept both.
        other => {
            let element = match other {
                Type::Array(element) => element.as_ref(),
                other => other,
            };
            let fill_to = parameter_count.max(leading.len() + 1);
            while expanded.len() < fill_to {
                expanded.push(element.clone());
            }
        }
    }
    expanded
}

pub(crate) fn check_arrow_function_expression(
    arrow: ParsedArrowFunction,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    check_arrow_function_expression_with_expected_type(arrow, None, symbols, ctx)
}

/// Emits the whole-signature mismatch tsc reports when a contextually-typed
/// arrow's returns do not fit: TS2322 anchored on the assignment target, or
/// TS2345 when the arrow is a call argument.
#[allow(clippy::too_many_arguments)]
fn emit_contextual_signature_mismatch(
    parameter_types: &[Type],
    returned_types: &[Type],
    parameters: &[surge_ts_syntax::ParsedFunctionParameter],
    is_variadic: bool,
    required_parameter_count: usize,
    expected_type: &FunctionType,
    span: Option<surge_ts_syntax::TextSpan>,
    is_argument: bool,
    ctx: &mut CheckerContext,
) {
    if returned_types.is_empty() {
        return;
    }
    // tsc widens the fresh literals a returned object literal carries, so
    // `return { ok: "nope" }` renders as `{ ok: string; }`.
    let returned =
        crate::checks::expr::widen_type(&surge_ts_types::union_type(returned_types.to_vec()));
    let source = alloc_function_type(
        parameter_types.to_vec(),
        returned,
        is_variadic,
        required_parameter_count,
    )
    .with_parameter_names(signature::written_binding_names(parameters));
    let source = Type::Function(source);
    let target = Type::Function(expected_type.clone());
    let (source_name, target_name) = crate::checks::expr::disambiguated_pair(
        &source,
        source.name(),
        &target,
        target.name(),
        &ctx.file_name,
    );
    let diagnostic = if is_argument {
        crate::checks::expr::assignability_mismatch_diagnostic(
            &source,
            &target,
            &source_name,
            &target_name,
            true,
            ctx.file_name.clone(),
        )
    } else {
        crate::checks::expr::type_not_assignable_diagnostic(
            &source,
            &target,
            &source_name,
            &target_name,
            ctx.file_name.clone(),
        )
    };
    ctx.push(crate::spans::diagnostic_with_syntax_span(diagnostic, span));
}

pub(crate) fn check_arrow_function_expression_with_expected_type(
    arrow: ParsedArrowFunction,
    expected_type: Option<&FunctionType>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    check_arrow_function_expression_anchored(arrow, expected_type, None, symbols, ctx)
}

/// [`check_arrow_function_expression_with_expected_type`] with the span tsc
/// anchors a whole-signature mismatch on — the assignment target, not the
/// failing return.
/// `SURGE_INFER_BLOCK_RETURN_TYPES=1`: infer an unannotated *block* body's
/// return type from its `return` statements, the way an expression body's
/// already is (tsc's `getReturnTypeFromBody`).
///
/// Measured 2026-09-16. This is the root of tRPC's router cluster, not proxy
/// modelling: `createTRPCNext({ config() { return opts; } })` cannot infer
/// `TRouter` because the method hands back the degradation sentinel. With this
/// on, an isolated probe matches tsc exactly and a directly-typed options value
/// closes both remaining tRPC false positives.
///
/// It is off because it is only half the fix. Function *declarations* infer
/// their return type on a different path and still degrade, so
/// `testServerAndClientResource(appRouter)` — and with it the real call site —
/// stays unresolved; and making the source concrete exposes a latent
/// false positive where the *target* still carries an unsubstituted type
/// parameter (`NextComponentType<…, AppPropsType<any, P>>`), which took trpc
/// from 2 to 33. zod 0/0, the sweep and every other corpus are unchanged.
fn infer_block_body_return_types() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED
        .get_or_init(|| std::env::var("SURGE_INFER_BLOCK_RETURN_TYPES").as_deref() == Ok("1"))
}

/// tsc's `getReturnTypeFromBody` widens a single literal return type unless the
/// contextual return type is literal-like for it (`isLiteralOfContextualType`),
/// so `() => 1` returns `number` while `(): 1 => 1` and `c ? "a" : "b"` keep
/// their literals. A contextual type surge could not settle keeps the literal.
fn widen_unit_return_type(body_type: Type, contextual_return_type: Option<&Type>) -> Type {
    if !matches!(
        body_type,
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_)
    ) {
        return body_type;
    }
    if contextual_return_type.is_some_and(|contextual| is_literal_of_contextual_type(&body_type, contextual)) {
        return body_type;
    }
    crate::checks::expr::widen_type(&body_type)
}

fn is_literal_of_contextual_type(candidate: &Type, contextual: &Type) -> bool {
    match contextual {
        Type::Unknown | Type::TypeParameter(_) => true,
        Type::Reference(reference) => is_literal_of_contextual_type(candidate, &reference.resolve()),
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| is_literal_of_contextual_type(candidate, member)),
        Type::StringLiteral(_) => matches!(candidate, Type::StringLiteral(_)),
        Type::NumberLiteral(_) => matches!(candidate, Type::NumberLiteral(_)),
        Type::BooleanLiteral(_) | Type::Boolean => matches!(candidate, Type::BooleanLiteral(_)),
        _ => false,
    }
}

/// tsc's `elaborateArrowFunction`: an expression-bodied arrow with no
/// annotated parameters whose body does not fit the contextual return type is
/// reported at the body, as the return types rather than the whole signatures.
fn report_contextual_body_mismatch(
    body_type: &Type,
    contextual_return_type: &Type,
    body_span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    let Some(body_span) = body_span else {
        return false;
    };
    if matches!(
        contextual_return_type,
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Void
    ) || surge_ts_types::is_assignable_to(body_type, contextual_return_type)
        || type_contains_unknown(body_type)
        || type_contains_unknown(contextual_return_type)
    {
        return false;
    }
    let reported_target =
        crate::checks::expr::reported_relation_target(body_type, contextual_return_type);
    let source_name = crate::checks::expr::source_display_name(body_type, &reported_target);
    let target_name = reported_target.name();
    let diagnostic = crate::checks::expr::type_not_assignable_diagnostic(
        body_type,
        &reported_target,
        &source_name,
        &target_name,
        ctx.file_name.clone(),
    )
    .with_span(crate::context::convert_span(body_span));
    ctx.push(diagnostic);
    true
}

pub(crate) fn check_arrow_function_expression_anchored(
    arrow: ParsedArrowFunction,
    expected_type: Option<&FunctionType>,
    target_span: Option<surge_ts_syntax::TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    let ParsedArrowFunction {
        is_generator,
        this_binding,
        type_parameters,
        parameters,
        return_type,
        return_type_span,
        is_async,
        body,
        body_reads,
        body_span,
        span: arrow_span,
    } = arrow;
    let is_argument = std::mem::take(&mut ctx.next_arrow_is_argument);

    // An arrow does not bind `this`, so it keeps whatever the enclosing function
    // established; a `function` expression and an object-literal method both
    // lower to this shape but bind their own.
    let outer_this_is_implicitly_any = ctx.this_is_implicitly_any;
    match this_binding {
        surge_ts_syntax::ParsedThisBinding::Inherited => {}
        surge_ts_syntax::ParsedThisBinding::Own => ctx.this_is_implicitly_any = false,
        surge_ts_syntax::ParsedThisBinding::ImplicitAny => ctx.this_is_implicitly_any = true,
    }

    let expanded_contextual_parameter_types = expected_type
        .map(|expected_type| contextual_parameter_types(expected_type, parameters.len()));
    let contextual_parameter_types = expanded_contextual_parameter_types.as_deref();
    let result = with_type_parameter_scope(&type_parameters, ctx, |ctx| {
        // Resolve the arrow's annotations against the value symbols visible at
        // the arrow site, mirroring `check_variable_declaration_against_symbols`:
        // `(x: typeof localConst) => …` must see the enclosing function body's
        // locals, which live in the scope stack and never reach `ctx.symbols`.
        let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
        let function_type = map_function_signature(
            &parameters,
            return_type.as_ref(),
            &type_parameters,
            contextual_parameter_types,
            ctx,
        );
        ctx.symbols = saved_symbols;
        let source_type_name = function_type.name();
        let has_explicit_return_type = return_type.is_some();
        let mut parameter_types = function_type.parameters().to_vec();
        let mut return_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
            function_type.return_type().clone()
        });

        if let Some(expected_type) = expected_type {
            for (index, parameter_type) in contextual_parameter_types
                .unwrap_or_default()
                .iter()
                .cloned()
                .enumerate()
            {
                if index < parameter_types.len() && parameters[index].declared_type.is_none() {
                    // A default initializer supplies the value whenever the
                    // caller passes `undefined`, so inside the function the
                    // parameter no longer includes it
                    // (`(fn, options = {}) => …` reads `options` as the
                    // object).
                    parameter_types[index] = if parameters[index].initializer.is_some() {
                        without_undefined(parameter_type)
                    } else {
                        parameter_type
                    };
                }
            }

            if !has_explicit_return_type {
                return_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                    expected_type.return_type().clone()
                });
            }

            if has_contextual_unknown_object_binding_pattern(
                &parameters,
                contextual_parameter_types,
            ) {
                let target_type_name = expected_type.name();
                let diagnostic =
                    Diagnostic::ts2345(&source_type_name, &target_type_name, ctx.file_name.clone());
                let diagnostic = match arrow_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };
                ctx.push(diagnostic);
            }
        }

        if ctx.skip_annotated_function_bodies && has_explicit_return_type {
            return alloc_function_type(
                parameter_types,
                return_type,
                function_type.is_variadic(),
                function_type.required_parameter_count(),
            )
            .with_parameter_names(signature::written_binding_names(&parameters));
        }

        let mut scopes =
            ScopeStack::from_root(symbols.clone_with_reason(TypeCopyReason::FunctionBodySetup));
        scopes.push_function_scope();
        // A `function` expression binds its own `this`; without a `this`
        // parameter it has no type, so the enclosing scope's `this` must not
        // leak in. tsc takes `this` from the contextual signature here — surge
        // does not model that, and reading the enclosing class instead reported
        // its members missing (`ws.addEventListener('message', function () {
        // this.send(…) })` inside a class).
        if matches!(this_binding, surge_ts_syntax::ParsedThisBinding::ImplicitAny) {
            scopes.insert_current(
                "this",
                crate::symbols::SymbolInfo {
                    ty: Type::Any,
                    kind: crate::symbols::SymbolKind::Const,
                    function_signature: None,
                },
            );
        }
        for (index, parameter) in parameters.iter().enumerate() {
            // The signature keeps surge's variadic slot, but the binding a
            // contextually typed `...rest` introduces holds the collection.
            let rest_binding_type = expected_type
                .filter(|_| parameter.rest && parameter.declared_type.is_none())
                .filter(|_| index + 1 == parameters.len())
                .and_then(|expected_type| contextual_rest_parameter_type(expected_type, index));
            let parameter_type = rest_binding_type
                .as_ref()
                .unwrap_or_else(|| parameter_types.get(index).unwrap_or(&Type::Any));
            insert_parameter_bindings(parameter, parameter_type, &mut scopes);
        }

        if should_track_unused_parameters(ctx) {
            emit_unused_parameters(&parameters, &body_reads, ctx);
        }

        let visible_symbols = visible_symbols(&scopes);
        for (index, parameter) in parameters.iter().enumerate() {
            if let Some(parameter_type) = parameter_types.get(index) {
                check_binding_pattern_defaults(
                    &parameter.binding_name,
                    parameter_type,
                    &visible_symbols,
                    ctx,
                );
            }
        }
        // tsc's `unwrapReturnType`: an async body is typed against the awaited
        // return type (`async (): Promise<R> => ({ … })` reads the literal
        // against `R`).
        let body_return_type = if is_async && !is_generator {
            crate::checks::call::awaited_type(&return_type)
        } else {
            return_type.clone()
        };
        match body {
            ParsedArrowFunctionBody::Expression(expression) => {
                let return_type_for_body = match &body_return_type {
                    Type::Any
                    | Type::Unknown
                    | Type::GenuineUnknown
                    | Type::TypeParameter(_)
                    | Type::Void => None,
                    ty => Some(ty),
                };
                let inferred_body = match return_type_for_body {
                    None => evaluate_expression(&expression, None, &visible_symbols, ctx),
                    // Only an annotated return type is checked through
                    // `checkReturnExpression`; a contextual one is related by the
                    // enclosing assignment, which does not split a conditional.
                    Some(return_type_for_body) if !has_explicit_return_type => {
                        super::expected::evaluate_expression_with_expected_type(
                            &expression,
                            None,
                            Some(return_type_for_body),
                            super::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
                            &visible_symbols,
                            ctx,
                        )
                    }
                    Some(return_type_for_body) => {
                        let inferred = crate::checks::expected::evaluate_return_expression_with_expected_type(
                            &expression,
                            body_span,
                            None,
                            return_type_for_body,
                            &visible_symbols,
                            ctx,
                        );
                        // tsc's `checkReturnExpression` relates an expression body
                        // to the annotation as a whole, at the body — awaited on
                        // both sides for an async one (`unwrapReturnType`).
                        let awaited_sides = match &inferred {
                            InferredExpression::Known(body_type) if is_async => Some((
                                crate::checks::call::awaited_type(body_type),
                                crate::checks::call::awaited_type(return_type_for_body),
                            )),
                            _ => None,
                        };
                        let related_sides = match (&awaited_sides, &inferred) {
                            (Some((body_type, return_type)), _) => Some((body_type, return_type)),
                            (None, InferredExpression::Known(body_type)) => {
                                Some((body_type, return_type_for_body))
                            }
                            _ => None,
                        };
                        if let Some((body_type, return_type_for_body)) = related_sides
                            && !body_type.is_unknown()
                            && !surge_ts_types::is_assignable_to(body_type, return_type_for_body)
                            && !type_contains_unknown(body_type)
                            && !type_contains_unknown(return_type_for_body)
                        {
                            let reported_target = crate::checks::expr::reported_relation_target(
                                body_type,
                                return_type_for_body,
                            );
                            let source_name =
                                crate::checks::expr::source_display_name(body_type, &reported_target);
                            let diagnostic = crate::checks::expr::type_not_assignable_diagnostic(
                                body_type,
                                &reported_target,
                                &source_name,
                                &reported_target.name(),
                                ctx.file_name.clone(),
                            );
                            ctx.push(crate::spans::diagnostic_with_syntax_span(diagnostic, body_span));
                        }
                        inferred
                    }
                };

                if !has_explicit_return_type {
                    if let InferredExpression::Known(body_type) = inferred_body {
                        if !body_type.is_unknown() {
                            if expected_type.is_some()
                                && !is_async
                                && parameters.iter().all(|parameter| parameter.declared_type.is_none())
                                && report_contextual_body_mismatch(
                                    &body_type,
                                    &return_type,
                                    body_span,
                                    ctx,
                                )
                            {
                                // Reported on the body: the arrow keeps the
                                // contextual return so the enclosing relation
                                // does not report it again.
                            } else {
                                return_type = widen_unit_return_type(
                                    body_type,
                                    expected_type.map(|expected| expected.return_type()),
                                );
                                if is_async && !is_generator {
                                    return_type = crate::checks::call::promise_of(&return_type, ctx);
                                }
                            }
                        }
                    }
                }
            }
            ParsedArrowFunctionBody::Block(statements) => {
                emit_unused_locals(&statements, &body_reads, ctx);
                let flow_facts = collect_function_flow_facts(&statements);
                let mut flow_state = FunctionFlowState::new(
                    flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
                );
                let body_flow = analyze_function_body_flow(&statements);
                let return_type_for_body = match &return_type {
                    Type::Any
                    | Type::Unknown
                    | Type::GenuineUnknown
                    | Type::TypeParameter(_)
                    | Type::Void => None,
                    ty => Some(ty),
                };
                // A contextual return type this arrow did not annotate is not a
                // per-return assignability target for tsc — see
                // `ContextualReturnFrame`.
                // One frame per FUNCTION body — `check_function_body` also runs
                // for every nested block, so the frame cannot live there or an
                // `if` would shadow its own function's.
                if !has_explicit_return_type && expected_type.is_some() {
                    ctx.activate_next_body_frame();
                }
                ctx.open_contextual_return_frame();
                let outer_async_body =
                    std::mem::replace(&mut ctx.in_async_body, is_async && !is_generator);
                check_function_body(
                    statements,
                    return_type_for_body,
                    &mut scopes,
                    &mut flow_state,
                    ctx,
                );
                ctx.in_async_body = outer_async_body;
                // The flow verdict is checked first: the return-type gate walks
                // the whole type, which on a large annotation is far costlier
                // than the body it guards.
                if has_explicit_return_type
                    && !is_generator
                    && !body_flow.guarantees_exit
                    && !body_flow.guarantees_value_return
                    && should_check_missing_return(&body_return_type)
                {
                    emit_missing_return_diagnostic(
                        body_flow,
                        &body_return_type,
                        return_type_span.or(arrow_span),
                        ctx,
                    );
                }
                // tsc reports a contextually-typed arrow whose returns do not fit
                // as one whole-signature mismatch on the assignment, with the leaf
                // as nested elaboration. Take the leaf verdicts back and render
                // that instead.
                if let Some(returned_types) = ctx.take_contextual_return_mismatch()
                    && let Some(expected_type) = expected_type
                {
                    emit_contextual_signature_mismatch(
                        &parameter_types,
                        &returned_types,
                        &parameters,
                        function_type.is_variadic(),
                        function_type.required_parameter_count(),
                        expected_type,
                        target_span.or(arrow_span),
                        is_argument,
                        ctx,
                    );
                }
                // tsc infers an unannotated block body's return type from its
                // `return` statements (`getReturnTypeFromBody`). surge did that
                // only for an expression body, so an object-literal method
                // (`{ config() { return opts; } }`, which lowers to this shape)
                // handed back the sentinel — and every type parameter inferred
                // through that return died with it, which is what left tRPC's
                // router unmodelled at `createTRPCNext({ config() { … } })`.
                // A degraded return stays the sentinel: inferring from it would
                // turn "could not model" into a decision.
                if infer_block_body_return_types()
                    && !has_explicit_return_type
                    && expected_type.is_none()
                {
                    let returned = ctx.body_return_types().to_vec();
                    if !returned.is_empty() && returned.iter().all(|ty| !ty.is_unknown()) {
                        let mut members = returned;
                        if !body_flow.guarantees_exit {
                            members.push(Type::Undefined);
                        }
                        return_type = surge_ts_types::union_type(members);
                    }
                }
                let returned_void_like = ctx.close_contextual_return_frame();

                let contextually_void = expected_type
                    .is_some_and(|expected_type| matches!(expected_type.return_type(), Type::Void));
                if !has_explicit_return_type
                    && !contextually_void
                    && !returned_void_like
                    && ctx.options.no_implicit_returns
                    && body_flow.contains_return_with_value
                    && !body_flow.guarantees_exit
                {
                    emit_implicit_return_diagnostic(arrow_span, ctx);
                }
            }
        }

        if expected_type
            .is_some_and(|expected_type| matches!(expected_type.return_type(), Type::Void))
            && !has_explicit_return_type
        {
            return_type = Type::Void;
        }

        alloc_function_type(
            parameter_types,
            return_type,
            function_type.is_variadic(),
            function_type.required_parameter_count(),
        )
        .with_parameter_names(signature::written_binding_names(&parameters))
    });

    ctx.this_is_implicitly_any = outer_this_is_implicitly_any;
    result
}
