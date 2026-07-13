//! Function/arrow signature mapping, parameter binding, and signature registration.

use super::*;

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedArrayBindingPattern, ParsedBindingName, ParsedFunctionBodyStatement,
    ParsedFunctionParameter, ParsedObjectBindingElement, ParsedObjectBindingPattern, ParsedType,
    ParsedTypeParameter, TextSpan,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use crate::arena::alloc_function_type;
use crate::checks::expr::evaluate_expression;
use crate::checks::var::widen_implicit_variable_initializer_type;
use crate::context::convert_span;
use crate::context::{CheckerContext, FileKind};
use crate::flow::{FunctionFlowState, analyze_function_body_flow, collect_function_flow_facts};
use crate::infer::{
    InferredExpression, TypeParameterSubstitution, map_parsed_type_with_substitution,
    report_duplicate_type_parameters,
};
use crate::symbols::{FunctionSignatureInfo, ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

pub(crate) fn emit_parameter_diagnostics(
    parameter: &ParsedFunctionParameter,
    contextual_type: Option<&Type>,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_implicit_any
        || parameter.declared_type.is_some()
        || parameter.initializer.is_some()
    {
        return;
    }

    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            if contextual_type.is_some() {
                return;
            }
            let diagnostic = Diagnostic::ts7006(name, ctx.file_name.clone());
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(*span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            if contextual_type.is_some() {
                return;
            }
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            if contextual_type.is_some() {
                return;
            }
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn emit_array_binding_pattern_diagnostics(
    pattern: &ParsedArrayBindingPattern,
    ctx: &mut CheckerContext,
) {
    for element in pattern.elements.iter().flatten() {
        emit_array_binding_element_diagnostic(element, ctx);
    }
}

fn emit_array_binding_element_diagnostic(
    binding_name: &ParsedBindingName,
    ctx: &mut CheckerContext,
) {
    match binding_name {
        ParsedBindingName::Identifier { name, span } => {
            let diagnostic = Diagnostic::ts7031(name, "any", ctx.file_name.clone());
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(*span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn emit_object_binding_pattern_diagnostics(
    pattern: &ParsedObjectBindingPattern,
    ctx: &mut CheckerContext,
) {
    for element in &pattern.elements {
        emit_object_binding_element_diagnostic(element, ctx);
    }
    // The `...rest` binding gets an (empty) object type, not implicit `any`, so
    // tsc emits no TS7031 for it even when the surrounding pattern is untyped.
}

pub(crate) fn emit_object_binding_element_diagnostic(
    element: &ParsedObjectBindingElement,
    ctx: &mut CheckerContext,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            let diagnostic = Diagnostic::ts7031(name, "any", ctx.file_name.clone());
            let span = (*span).or(element.name_span);
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            emit_array_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn parameter_identifier_name(parameter: &ParsedFunctionParameter) -> Option<&str> {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(crate) fn parameter_scope_type(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
) -> Type {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { .. } => {
            let ty =
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone());
            // An optional parameter (`x?: T`) is `T | undefined` inside the body,
            // so comparing it to `undefined` is intentional and `=> undefined`
            // flows through narrowing. A defaulted parameter (`x: T = …`) is not
            // optional in the body (the default fills the gap), and a rest
            // parameter is already an array, so neither widens. This only affects
            // the in-body view; the signature's parameter type (used to check
            // call arguments) is unchanged.
            if parameter.optional && parameter.initializer.is_none() && !parameter.rest {
                surge_ts_types::union_type(vec![ty, Type::Undefined])
            } else {
                ty
            }
        }
        ParsedBindingName::ObjectPattern(_)
        | ParsedBindingName::ArrayPattern(_)
        | ParsedBindingName::Unsupported { .. } => Type::Any,
    }
}

pub(crate) fn insert_binding_name(
    binding_name: &ParsedBindingName,
    ty: Type,
    scopes: &mut ScopeStack,
) {
    match binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.as_str(),
                SymbolInfo {
                    ty,
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, ty, scopes);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            insert_array_binding_pattern_bindings(pattern, ty, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

/// The element type a destructuring position reads: the tuple element at `index`
/// for a tuple source, the element type for an array source, and `any`/`unknown`
/// otherwise (conservative — keeps the binding in scope without cascading).
fn array_binding_element_type(source: &Type, index: usize) -> Type {
    match source {
        Type::Tuple(elements) => elements.get(index).cloned().unwrap_or(Type::Undefined),
        Type::Array(element) => (**element).clone(),
        Type::Any => Type::Any,
        _ => Type::Any,
    }
}

pub(crate) fn insert_array_binding_pattern_bindings(
    pattern: &ParsedArrayBindingPattern,
    source_type: Type,
    scopes: &mut ScopeStack,
) {
    for (index, element) in pattern.elements.iter().enumerate() {
        if let Some(element) = element {
            let element_type = array_binding_element_type(&source_type, index);
            insert_binding_name(element, element_type, scopes);
        }
    }
    if let Some(rest) = &pattern.rest {
        // `[a, ...rest]` binds `rest` to the remaining elements. We model it as
        // an array of the source element type (or the source as-is) — precise
        // enough to keep `rest` usable without an exact `slice` shape.
        let rest_type = match &source_type {
            Type::Array(_) => source_type.clone(),
            Type::Tuple(elements) => {
                Type::Array(Box::new(elements.last().cloned().unwrap_or(Type::Any)))
            }
            _ => Type::Any,
        };
        insert_binding_name(rest, rest_type, scopes);
    }
}

pub(crate) fn insert_parameter_bindings(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
    scopes: &mut ScopeStack,
) {
    insert_binding_name(
        &parameter.binding_name,
        parameter_scope_type(parameter, parameter_type),
        scopes,
    );
}

pub(crate) fn insert_object_binding_pattern_bindings(
    pattern: &ParsedObjectBindingPattern,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    for element in &pattern.elements {
        insert_object_binding_element_binding(
            element,
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone()),
            scopes,
        );
    }
    // `{ a, ...rest }` binds `rest` to the remaining properties. The exact
    // `Omit<T, ...>` shape is not modelled; binding it to the source type keeps
    // `rest` in scope (and spreadable via `{...rest}`) without a TS2304 cascade.
    if let Some(rest) = &pattern.rest {
        insert_binding_name(
            rest,
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone()),
            scopes,
        );
    }
}

pub(crate) fn insert_object_binding_element_binding(
    element: &ParsedObjectBindingElement,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.as_str(),
                SymbolInfo {
                    ty: parameter_type,
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, parameter_type, scopes);
        }
        ParsedBindingName::ArrayPattern(pattern) => {
            insert_array_binding_pattern_bindings(pattern, parameter_type, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

pub(crate) fn map_function_signature(
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    contextual_parameter_types: Option<&[Type]>,
    ctx: &mut CheckerContext,
) -> FunctionType {
    report_duplicate_type_parameters(type_parameters, ctx);

    // Register the signature's type parameters (with their constraints) for the
    // duration of parameter/return-type resolution. The placeholder substitution
    // alone marks `K` as generic, but the *constraint* (`K extends keyof Hooks`)
    // lives only in this scope; without it a constrained indexed access in the
    // return type (`Required<Hooks>[K]`) cannot be recognised as a valid generic
    // index and degrades to a false `TS2536`.
    let pushed_type_parameter_scope = !type_parameters.is_empty();
    if pushed_type_parameter_scope {
        ctx.push_type_parameter_scope(type_parameters, None);
    }

    let type_parameter_substitution = build_type_parameter_substitution(type_parameters);
    let mut parameter_types = Vec::with_capacity(parameters.len());
    let mut parameter_symbols = None;
    let mut parameter_bindings: Vec<(String, Type)> = Vec::new();

    for (index, parameter) in parameters.iter().enumerate() {
        let inferred_parameter_type = if let Some(declared_type) = parameter.declared_type.clone() {
            map_parsed_type_with_substitution(declared_type, ctx, &type_parameter_substitution)
        } else if let Some(initializer) = parameter.initializer.as_ref() {
            let parameter_symbols = parameter_symbols.get_or_insert_with(|| {
                let mut symbols = ctx
                    .symbols
                    .clone_with_reason(TypeCopyReason::FunctionBodySetup);
                for (name, ty) in &parameter_bindings {
                    let _ = symbols.insert(
                        name.clone(),
                        SymbolInfo {
                            ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                                ty.clone()
                            }),
                            kind: SymbolKind::Parameter,
                            function_signature: None,
                        },
                    );
                }
                symbols
            });
            let inferred_initializer = evaluate_expression(
                initializer,
                parameter.initializer_span,
                parameter_symbols,
                ctx,
            );

            match inferred_initializer {
                InferredExpression::Known(ty) => {
                    widen_implicit_variable_initializer_type(SymbolKind::Let, &ty)
                }
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => Type::Unknown,
            }
        } else {
            Type::Any
        };

        if let Some(name) = parameter_identifier_name(parameter) {
            let parameter_binding_type =
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                    inferred_parameter_type.clone()
                });
            if let Some(parameter_symbols) = parameter_symbols.as_mut() {
                let _ = parameter_symbols.insert(
                    name.to_string(),
                    SymbolInfo {
                        ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                            parameter_binding_type.clone()
                        }),
                        kind: SymbolKind::Parameter,
                        function_signature: None,
                    },
                );
            }
            parameter_bindings.push((name.to_string(), parameter_binding_type));
        }

        parameter_types.push(inferred_parameter_type);

        if ctx.options.no_implicit_any {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
        }
    }

    let function_return_type = return_type
        .map(|return_type| {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                map_parsed_type_with_substitution(
                    return_type.clone(),
                    ctx,
                    &type_parameter_substitution,
                )
            })
        })
        .unwrap_or(Type::Unknown);

    if pushed_type_parameter_scope {
        ctx.pop_type_parameter_scope();
    }

    alloc_function_type(
        parameter_types,
        function_return_type,
        parameters.last().is_some_and(|parameter| parameter.rest),
        required_parameter_count(parameters),
    )
}

pub(crate) fn required_parameter_count(parameters: &[ParsedFunctionParameter]) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.initializer.is_some() || parameter.rest {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

pub(crate) fn has_contextual_unknown_object_binding_pattern(
    parameters: &[ParsedFunctionParameter],
    contextual_parameter_types: Option<&[Type]>,
) -> bool {
    let Some(contextual_parameter_types) = contextual_parameter_types else {
        return false;
    };

    parameters.iter().enumerate().any(|(index, parameter)| {
        matches!(parameter.binding_name, ParsedBindingName::ObjectPattern(_))
            && parameter.declared_type.is_none()
            && contextual_parameter_types
                .get(index)
                .is_some_and(|ty| ty.is_unknown())
    })
}

pub(crate) fn build_type_parameter_substitution(
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();

    for type_parameter in type_parameters {
        substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
    }

    substitution
}

pub(crate) fn function_signature_info(
    type_parameters: &[ParsedTypeParameter],
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    declaring_file: &str,
) -> FunctionSignatureInfo {
    FunctionSignatureInfo {
        type_parameters: type_parameters.to_vec(),
        parameter_types: parameters
            .iter()
            .map(|parameter| parameter.declared_type.clone())
            .collect(),
        return_type: return_type.cloned(),
        declaring_file: Some(declaring_file.to_string()),
    }
}

pub(crate) fn with_type_parameter_scope<R>(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let mut scope = std::collections::HashMap::new();
    for type_parameter in type_parameters {
        scope.insert(type_parameter.name.clone(), Type::Unknown);
    }

    ctx.push_type_parameter_scope(type_parameters, Some(scope));
    let result = f(ctx);
    ctx.pop_type_parameter_scope();
    result
}

pub(crate) fn register_function_signature(
    name: String,
    function_type: FunctionType,
    function_signature: Option<FunctionSignatureInfo>,
    symbols: &mut SymbolTable,
    replace_existing: bool,
    is_implementation: bool,
) -> bool {
    let symbol_exists = matches!(
        symbols.get(&name),
        Some(existing) if matches!(existing.kind, SymbolKind::Function)
    );

    // TS2393 ("Duplicate function implementation") fires only when *this*
    // declaration has a body and another implementation was already registered.
    // Bodyless declarations (overload signatures, ambient `declare function`s)
    // merge as overloads, so two of them — or an overload preceding an
    // implementation — is not a duplicate.
    let duplicate_implementation = is_implementation && symbols.has_function_implementation(&name);
    if is_implementation {
        symbols.mark_function_implementation(&name);
    }

    if symbol_exists && !replace_existing {
        return duplicate_implementation;
    }

    if !symbol_exists || replace_existing {
        symbols.insert(
            name,
            SymbolInfo {
                ty: Type::Function(function_type),
                kind: SymbolKind::Function,
                function_signature,
            },
        );
    }

    duplicate_implementation
}

/// Whether `noUnusedParameters` reporting applies in the current file. Skips
/// declaration files (ambient / `.d.ts`), where tsc never reports.
pub(crate) fn should_track_unused_parameters(ctx: &CheckerContext) -> bool {
    ctx.options.no_unused_parameters && ctx.current_file_kind == FileKind::RootSource
}

/// Reports TS6133 for each identifier parameter whose name never appears in the
/// body's collected reads (and is not `_`-prefixed). Object/array patterns and
/// the `this` pseudo-parameter are skipped.
pub(crate) fn emit_unused_parameters(
    parameters: &[ParsedFunctionParameter],
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    for parameter in parameters {
        let ParsedBindingName::Identifier { name, span } = &parameter.binding_name else {
            continue;
        };
        if name == "this" || name.starts_with('_') || reads.iter().any(|read| read == name) {
            continue;
        }
        let diagnostic = Diagnostic::ts6133(name, ctx.file_name.clone());
        let diagnostic = match span {
            Some(span) => diagnostic.with_span(convert_span(*span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// Reports TS6133 for each function-local `const`/`let`/`var` whose name never
/// appears in the body's reads. Gated on `noUnusedLocals` in a root source file.
/// Uses the function-wide read set, so a binding read in any nested scope counts
/// (an over-approximation — never a false positive).
pub(crate) fn emit_unused_locals(
    statements: &[ParsedFunctionBodyStatement],
    reads: &[String],
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_unused_locals || ctx.current_file_kind != FileKind::RootSource {
        return;
    }
    let mut locals: Vec<(&str, Option<TextSpan>)> = Vec::new();
    collect_local_var_declarations(statements, &mut locals);
    for (name, span) in locals {
        if reads.iter().any(|read| read == name) {
            continue;
        }
        let diagnostic = Diagnostic::ts6133(name, ctx.file_name.clone());
        let diagnostic = match span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// Collects `const`/`let`/`var` declarations directly owned by this function
/// body, recursing through control-flow statements but not into nested functions
/// (whose locals belong to their own scope).
fn collect_local_var_declarations<'a>(
    statements: &'a [ParsedFunctionBodyStatement],
    out: &mut Vec<(&'a str, Option<TextSpan>)>,
) {
    for statement in statements {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) if !variable.is_declare => {
                out.push((variable.name.as_str(), variable.name_span));
            }
            ParsedFunctionBodyStatement::Block(body) => collect_local_var_declarations(body, out),
            ParsedFunctionBodyStatement::If(statement) => {
                collect_local_var_declarations(&statement.then_body, out);
                collect_local_var_declarations(&statement.else_body, out);
            }
            ParsedFunctionBodyStatement::While(statement) => {
                collect_local_var_declarations(&statement.body, out)
            }
            ParsedFunctionBodyStatement::ForOf(statement) => {
                collect_local_var_declarations(&statement.body, out)
            }
            ParsedFunctionBodyStatement::Switch(statement) => {
                for case in &statement.cases {
                    collect_local_var_declarations(&case.consequent, out);
                }
            }
            ParsedFunctionBodyStatement::Try(statement) => {
                collect_local_var_declarations(&statement.block, out);
                if let Some(handler) = &statement.handler {
                    collect_local_var_declarations(&handler.body, out);
                }
                collect_local_var_declarations(&statement.finalizer, out);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_function_body_with_signature(
    name: String,
    parameters: Vec<ParsedFunctionParameter>,
    body: Vec<ParsedFunctionBodyStatement>,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    function_signature: Option<FunctionSignatureInfo>,
    has_explicit_return_type: bool,
    missing_return_span: Option<TextSpan>,
    body_reads: Option<&[String]>,
    ctx: &mut CheckerContext,
) {
    check_function_body_with_signature_and_this(
        name,
        parameters,
        body,
        function_type,
        type_parameters,
        function_signature,
        has_explicit_return_type,
        missing_return_span,
        None,
        false,
        body_reads,
        ctx,
    );
}

/// Like [`check_function_body_with_signature`], but optionally binds a `this`
/// symbol (the class instance or static side) into the body scope so class
/// method and constructor bodies can resolve `this.<member>` references.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_function_body_with_signature_and_this(
    name: String,
    parameters: Vec<ParsedFunctionParameter>,
    body: Vec<ParsedFunctionBodyStatement>,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    function_signature: Option<FunctionSignatureInfo>,
    has_explicit_return_type: bool,
    missing_return_span: Option<TextSpan>,
    this_type: Option<Type>,
    is_constructor: bool,
    body_reads: Option<&[String]>,
    ctx: &mut CheckerContext,
) {
    let body_flow = analyze_function_body_flow(&body);
    let flow_facts = collect_function_flow_facts(&body);

    let mut scopes = ScopeStack::from_root(merged_function_body_root_symbols(ctx));
    scopes.insert_current(
        name,
        SymbolInfo {
            ty: Type::Function(with_type_copy_reason(
                TypeCopyReason::FunctionBodySetup,
                || function_type.clone(),
            )),
            kind: SymbolKind::Function,
            function_signature,
        },
    );
    if let Some(this_type) = this_type {
        scopes.insert_current(
            "this".to_string(),
            SymbolInfo {
                ty: this_type,
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
    scopes.push_child();
    let mut flow_state = FunctionFlowState::new(
        flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
    );

    // `None` is an overload signature (no body); tsc never flags its parameters
    // or locals.
    if let Some(reads) = body_reads {
        if !is_constructor && should_track_unused_parameters(ctx) {
            emit_unused_parameters(&parameters, reads, ctx);
        }
        emit_unused_locals(&body, reads, ctx);
    }

    for (parameter, parameter_type) in parameters
        .into_iter()
        .zip(function_type.parameters().iter())
    {
        insert_parameter_bindings(&parameter, parameter_type, &mut scopes);
    }

    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        check_function_body(
            body,
            Some(function_type.return_type()),
            &mut scopes,
            &mut flow_state,
            ctx,
        );
    });

    if has_explicit_return_type && should_check_missing_return(function_type.return_type()) {
        emit_missing_return_diagnostic(body_flow, missing_return_span, ctx);
    } else if !has_explicit_return_type
        && !is_constructor
        && ctx.options.no_implicit_returns
        && body_flow.contains_return_with_value
        && !body_flow.guarantees_exit
    {
        emit_implicit_return_diagnostic(missing_return_span, ctx);
    }
}

pub(crate) fn merged_function_body_root_symbols(ctx: &CheckerContext) -> SymbolTable {
    // A function body sees its module's symbols layered over the ambient globals.
    // Rather than copying every visible symbol into a fresh table on each function
    // (O(module symbols) per function, so O(N^2) for a file with N functions), share
    // them by `Arc` as a lookup-only parent chain: locals -> module -> ambient. The
    // scope still inserts its own name/params/locals into the (empty) own map, so
    // lookups and shadowing behave exactly as with the previously flattened table.
    let ambient = Arc::new(
        ctx.ambient_global_symbols
            .clone_with_reason(TypeCopyReason::FunctionBodySetup),
    );
    let module_over_ambient = Arc::new(
        ctx.symbols
            .clone_with_reason(TypeCopyReason::FunctionBodySetup)
            .with_parent_fallback(ambient),
    );
    SymbolTable::with_parent(module_over_ambient)
}
