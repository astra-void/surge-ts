use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, ParsedVariableDeclaration};
use surge_ts_types::{Type, TypeCopyReason, is_assignable_to};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type_anchored};
use super::expr::{evaluate_expression, source_display_name, widen_type};
use crate::context::{CheckerContext, convert_span};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::{SymbolInfo, SymbolInfoHandle, SymbolKind, SymbolTable, map_symbol_kind};

pub(crate) struct VariableCheckOptions {
    pub(crate) report_duplicate_let_const: bool,
    pub(crate) check_initializer: bool,
}

pub(crate) fn check_variable_declaration(
    variable: ParsedVariableDeclaration,
    ctx: &mut CheckerContext,
) {
    let mut symbols = std::mem::take(&mut ctx.symbols);

    check_variable_declaration_with_symbols(
        variable,
        &mut symbols,
        ctx,
        VariableCheckOptions {
            report_duplicate_let_const: true,
            check_initializer: true,
        },
    );

    ctx.symbols = symbols;
}

pub(crate) fn check_variable_declaration_with_symbols(
    variable: ParsedVariableDeclaration,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
    options: VariableCheckOptions,
) -> Option<SymbolInfoHandle> {
    let variable_name = variable.name.clone();
    let variable_name_span = variable.name_span;
    let tracks_duplicates = options.report_duplicate_let_const
        && !variable.is_declare
        && matches!(
            map_symbol_kind(variable.kind),
            SymbolKind::Let | SymbolKind::Const
        );
    let is_duplicate = tracks_duplicates && symbols.contains_let_or_const(&variable_name);

    let symbol = check_variable_declaration_against_symbols(variable, symbols, ctx, options)?;

    if is_duplicate {
        if let Some(first_span) = symbols.take_declaration_span(&variable_name) {
            let diagnostic = Diagnostic::ts2451(&variable_name, ctx.file_name.clone())
                .with_span(convert_span(first_span));
            ctx.push(diagnostic);
        }
    } else if tracks_duplicates {
        if let Some(span) = variable_name_span {
            symbols.record_declaration_span(&variable_name, span);
        }
    }

    symbols.insert_handle(variable_name, Arc::clone(&symbol));
    Some(symbol)
}

/// Reports an initializer whose type is not assignable to the annotation it
/// initializes, at `target_span` — the declared name.
pub(crate) fn report_initializer_mismatch(
    inferred_initializer_type: &Type,
    declared_type: &Type,
    target_span: Option<surge_ts_syntax::TextSpan>,
    ctx: &mut CheckerContext,
) {
    let definite_mismatch = crate::checks::assign::definite_primitive_member_mismatch(
        inferred_initializer_type,
        declared_type,
    );
    if inferred_initializer_type.is_unmodelled()
        || (type_contains_unknown(declared_type) && !definite_mismatch)
        || crate::checks::call::as_source(|| type_contains_unknown(inferred_initializer_type))
        || crate::checks::call::is_open_instantiation(inferred_initializer_type)
        || is_assignable_to(inferred_initializer_type, declared_type)
    {
        return;
    }
    let declared_type = &crate::checks::expr::reported_relation_target(
        inferred_initializer_type,
        declared_type,
    );
    let inferred_type_name = source_display_name(inferred_initializer_type, declared_type);
    let declared_type_name = declared_type.name();
    let (inferred_type_name, declared_type_name) = crate::checks::expr::disambiguated_pair(
        inferred_initializer_type,
        inferred_type_name,
        declared_type,
        declared_type_name,
        &ctx.file_name,
    );
    let diagnostic = crate::checks::expr::assignability_mismatch_diagnostic(
        inferred_initializer_type,
        declared_type,
        &inferred_type_name,
        &declared_type_name,
        false,
        ctx.file_name.clone(),
    );
    ctx.push(match target_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

pub(crate) fn check_variable_declaration_against_symbols(
    variable: ParsedVariableDeclaration,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
    options: VariableCheckOptions,
) -> Option<SymbolInfoHandle> {
    // Resolve the declared type against the in-scope value symbols so a
    // `typeof <value>` annotation can see locals/globals. The variable checker
    // moves `ctx.symbols` out into `symbols`, so without this the typeof lookup
    // would run against an empty table and spuriously report TS2304.

    let variable_name = variable.name.clone();
    let variable_name_span = variable.name_span;
    let redeclaration_candidate = !variable.is_declare && variable.declared_type.is_some();
    let auto_array = is_auto_array_candidate(&variable, ctx);

    // A generic annotation is kept for call-site instantiation; a type-predicate
    // annotation is kept so `if (isFoo(x))` can narrow — neither is recoverable
    // from the resolved callable type alone.
    let declared_function_type = match &variable.declared_type {
        Some(surge_ts_syntax::ParsedType::Function(function_type))
            if !function_type.type_parameters.is_empty()
                || matches!(
                    function_type.return_type.as_ref(),
                    surge_ts_syntax::ParsedType::Predicate(_)
                ) =>
        {
            Some(function_type.clone())
        }
        _ => None,
    };
    let declared_type = variable.declared_type.map(|declared_type| {
        let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
        let resolved = map_parsed_type(declared_type, ctx);
        ctx.symbols = saved_symbols;
        resolved
    });

    let symbol_kind = map_symbol_kind(variable.kind);

    // A `declare const`/`declare let` is pre-registered as an ambient symbol
    // before this check runs, so the duplicate probe would always find the
    // declaration's own pre-registration and report a spurious redeclaration.
    // Ambient declarations do not conflict with themselves; skip the report.
    // A module-scoped `let`/`const` shadows an ambient global of the same name
    // (`let name = …` over the DOM `declare var name`) without conflict — tsc
    // only reports a redeclaration when both declarations share a scope. The
    // merged symbol table folds ambient globals in flat, so exclude a name that
    // is solely an ambient global here; a genuine same-scope redeclaration still
    // reports through the span-tracked path in `check_variable_declaration`.
    if options.report_duplicate_let_const
        && !variable.is_declare
        && matches!(symbol_kind, SymbolKind::Let | SymbolKind::Const)
        && symbols.contains_let_or_const(&variable.name)
        && ctx.ambient_global_symbols.get(&variable.name).is_none()
    {
        let diagnostic = Diagnostic::ts2451(&variable.name, ctx.file_name.clone());
        let diagnostic = match variable.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
    }

    // Inside its own initializer an annotated binding already has its declared
    // type: tsc's `getTypeOfVariableOrParameterOrProperty` answers the
    // annotation whatever is being checked, so a deferred self-reference
    // (`const s: ZodType<T> = z.lazy(() => s)`) reads `ZodType<T>`. The caller
    // pre-binds the name to the sentinel for the initializer; swap in the
    // annotation resolved just above, so it is still resolved exactly once.
    let self_bound_symbols = declared_type
        .as_ref()
        .filter(|declared_type| !declared_type.is_unknown())
        .filter(|_| {
            options.check_initializer
                && variable.initializer.is_some()
                && symbols
                    .get(&variable_name)
                    .is_some_and(|symbol| matches!(symbol.ty, Type::Unknown))
        })
        .map(|declared_type| {
            let mut bound = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
            let _ = bound.insert(
                variable_name.clone(),
                SymbolInfo {
                    ty: declared_type.clone(),
                    kind: symbol_kind,
                    function_signature: None,
                },
            );
            bound
        });
    let initializer_symbols = self_bound_symbols.as_ref().unwrap_or(symbols);

    // tsc checks an array pattern's source once for iterability, and a
    // source without the protocol types every element `any`.
    let non_iterable_pattern_source = options.check_initializer
        && variable
            .array_pattern_span
            .zip(variable.initializer.as_ref().and_then(array_pattern_source))
            .is_some_and(|(pattern_span, source)| {
                let InferredExpression::Known(source_type) = crate::infer::infer_expression(&source, symbols, ctx)
                else {
                    return false;
                };
                if !crate::checks::expr::is_definitely_not_iterable(&source_type, true) {
                    return false;
                }
                let diagnostic = Diagnostic::ts2488(source_type.name(), ctx.file_name.clone())
                    .with_span(convert_span(pattern_span));
                ctx.push(diagnostic);
                true
            });

    let outer_allow_missing = ctx.allow_missing_tuple_element;
    ctx.allow_missing_tuple_element = variable.from_binding_pattern
        && matches!(
            variable.initializer,
            Some(ParsedExpression::NullishCoalescing { .. })
        );
    // `const s: unique symbol = Symbol()` is what creates that unique symbol:
    // the call's `symbol` is this declaration's own type, not a mismatch.
    let initializer_target = declared_type.as_ref().filter(|declared_type| {
        !(matches!(
            declared_type,
            Type::Reference(reference)
                if reference.is_unique_symbol()
                    && reference.id.ends_with(&format!("\u{0}{}", variable.name))
        ) && variable
            .initializer
            .as_ref()
            .is_some_and(is_symbol_constructor_call))
    });
    let inferred_initializer = if non_iterable_pattern_source {
        InferredExpression::Known(Type::Any)
    } else if options.check_initializer {
        variable
            .initializer
            .as_ref()
            .map(|initializer| {
                if let Some(declared_type) = initializer_target {
                    evaluate_expression_with_expected_type_anchored(
                        initializer,
                        variable.initializer_span,
                        variable.name_span,
                        Some(declared_type),
                        ExpectedTypeDiagnostic::TypeNotAssignable,
                        initializer_symbols,
                        ctx,
                    )
                } else {
                    evaluate_expression(initializer, variable.initializer_span, symbols, ctx)
                }
            })
            .unwrap_or(InferredExpression::Unknown)
    } else {
        InferredExpression::Unknown
    };
    ctx.allow_missing_tuple_element = outer_allow_missing;
    let mut inferred_symbol_type = match &inferred_initializer {
        InferredExpression::Known(inferred_initializer_type) => {
            if let Some(declared_type) = initializer_target {
                report_initializer_mismatch(
                    inferred_initializer_type,
                    declared_type,
                    variable.name_span.or(variable.initializer_span),
                    ctx,
                );
            }

            // Only the check phase publishes an error-typed binding: during
            // analysis the initializer may be the error type because an import
            // it names is not bound yet, and an export typed from that would
            // reach every consumer.
            if declared_type.is_none() && matches!(inferred_initializer_type, Type::ErrorType) {
                Some(crate::infer::unresolved_name_error_type().unwrap_or(Type::Unknown))
            } else if declared_type.is_none()
                && !inferred_initializer_type.is_unknown()
                && let Some(initializer) = variable.initializer.as_ref()
            {
                Some(widen_implicit_variable_initializer_type(
                    symbol_kind,
                    initializer,
                    inferred_initializer_type,
                    auto_array,
                ))
            } else {
                declared_type.clone().or(Some(Type::Unknown))
            }
        }
        // The binding of a failed lookup is tsc's error type too.
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. } => declared_type
            .clone()
            .or_else(|| inferred_initializer.clone().flowing_type())
            .or(Some(Type::Unknown)),
        InferredExpression::Unknown => declared_type.clone().or(Some(Type::Unknown)),
    };

    if declared_type.is_none() && variable.initializer.is_none() {
        inferred_symbol_type = Some(Type::Any);
    }

    // A generic arrow assigned to a binding (`const arrayToEnum = <T, U>(…) => …`,
    // the shape every namespace-scoped helper takes) is still a generic call
    // target: without its signature the call site cannot infer type arguments and
    // the result degrades to the sentinel. Only the un-annotated form carries it;
    // an explicit annotation already supplies the callable type.
    let function_signature = match (&declared_type, variable.initializer.as_ref()) {
        // A *type predicate* is carried the same way, generic or not: the guard
        // machinery reads `v is T` off the written signature, and an inferred
        // function type has nowhere to keep it — so `const isW = (v: W | X): v
        // is W => …` narrowed nothing at all, while the same predicate written
        // as a `function` declaration did.
        (None, Some(surge_ts_syntax::ParsedExpression::ArrowFunction(arrow)))
            if !arrow.type_parameters.is_empty()
                || matches!(arrow.return_type, Some(surge_ts_syntax::ParsedType::Predicate(_))) =>
        {
            Some(crate::checks::function::function_signature_info(
                &arrow.type_parameters,
                &arrow.parameters,
                arrow.return_type.as_ref(),
                &ctx.file_name,
            ))
        }
        _ => None,
    };
    // `const isStr = (x: string | number) => typeof x === "string"` implies
    // its predicate exactly as the `function` spelling does.
    let function_signature = function_signature.or_else(|| {
        let (None, Some(surge_ts_syntax::ParsedExpression::ArrowFunction(arrow))) =
            (&declared_type, variable.initializer.as_ref())
        else {
            return None;
        };
        if arrow.return_type.is_some() || arrow.is_async || arrow.is_generator {
            return None;
        }
        let Some(Type::Function(function_type)) = inferred_symbol_type.as_ref() else {
            return None;
        };
        let returned = match &arrow.body {
            surge_ts_syntax::ParsedArrowFunctionBody::Expression(expression) => expression,
            surge_ts_syntax::ParsedArrowFunctionBody::Block(statements) => {
                crate::checks::function::single_returned_statement_expression(statements)?
            }
        };
        let inferred = crate::checks::function::infer_predicate_from_body(
            &arrow.parameters,
            function_type.parameters(),
            returned,
            symbols,
        )?;
        Some(crate::checks::function::with_inferred_predicate(
            crate::checks::function::function_signature_info(
                &arrow.type_parameters,
                &arrow.parameters,
                None,
                &ctx.file_name,
            ),
            Some(inferred),
        ))
    });
    // An explicit *generic* function-type annotation supplies the callable
    // shape, but not the parsed return annotation a call with explicit type
    // arguments needs to re-resolve.
    let function_signature = function_signature.or_else(|| {
        declared_function_type.as_ref().map(|function_type| {
            crate::checks::function::function_type_signature_info(function_type, &ctx.file_name)
        })
    });

    // Go carries an unresolved import's error type through every binding that
    // reads from it, so `const q = trpc.post.all.useQuery()` is `any` there too
    // and a callback passed to a call on `q` has no contextual type. surge's
    // `Type::Any` cannot say which `any` it is, so the provenance is recorded
    // beside the binding instead — otherwise it stopped at the import and every
    // downstream call read as a chain surge merely failed to model.
    if declared_type.is_none()
        && let Some(initializer) = variable.initializer.as_ref()
    {
        if crate::checks::call::property::receiver_any_is_genuine(initializer, symbols, ctx) {
            ctx.genuine_any_bindings.insert(variable_name.clone());
        } else {
            ctx.genuine_any_bindings.remove(variable_name.as_str());
        }
    }

    let symbol = declared_type.or(inferred_symbol_type).map(|ty| {
        Arc::new(SymbolInfo {
            ty,
            kind: symbol_kind,
            function_signature,
        })
    })?;

    if redeclaration_candidate {
        report_redeclared_var_type(&variable_name, variable_name_span, &symbol, symbols, ctx);
    }

    Some(symbol)
}

/// TS2403: a `var` may be redeclared, but every declaration has to give it the
/// same type. `get_own` keeps this to the declaring scope, so a module `var`
/// that shadows a same-named ambient global is not a redeclaration.
///
/// Both declarations must be annotated. tsc compares the *widened declaration*
/// types, which for an unannotated `var` comes from its initializer; surge's
/// inference is not faithful enough there to report on it, so an unannotated
/// declaration on either side leaves the pair alone.
///
/// An ambient `declare var` is skipped for the reason the duplicate `let`/`const`
/// check skips it: it is pre-registered before this runs, so its own
/// registration would read as the earlier declaration.
fn report_redeclared_var_type(
    variable_name: &str,
    variable_name_span: Option<surge_ts_syntax::TextSpan>,
    symbol: &SymbolInfoHandle,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if !matches!(symbol.kind, SymbolKind::Var) {
        return;
    }
    let Some(previous) = symbols
        .get_own(variable_name)
        .filter(|existing| matches!(existing.kind, SymbolKind::Var))
        .map(|existing| existing.ty.clone())
    else {
        return;
    };
    if previous.is_unknown() || symbol.ty.is_unknown() || previous == symbol.ty {
        return;
    }

    let diagnostic = Diagnostic::ts2403(
        variable_name,
        previous.name(),
        symbol.ty.name(),
        ctx.file_name.clone(),
    );
    ctx.push(match variable_name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

/// A declaration tsc gives a control-flow tracked `any[]` (`autoArrayType`):
/// an un-annotated, non-ambient, non-destructured variable initialized with
/// `[]`, under `noImplicitAny`.
pub(crate) fn is_auto_array_candidate(
    variable: &surge_ts_syntax::ParsedVariableDeclaration,
    ctx: &CheckerContext,
) -> bool {
    ctx.options.no_implicit_any
        && variable.declared_type.is_none()
        && !variable.is_declare
        && !variable.from_binding_pattern
        && matches!(
            &variable.initializer,
            Some(ParsedExpression::ArrayLiteral { elements, .. }) if elements.is_empty()
        )
}

pub(crate) fn widen_implicit_variable_initializer_type(
    symbol_kind: SymbolKind,
    initializer: &ParsedExpression,
    ty: &Type,
    auto_array: bool,
) -> Type {
    // `getWidenedTypeForVariableLikeDeclaration`: under `noImplicitAny` an
    // empty array initializer makes an evolving array (`autoArrayType`, read
    // as `any[]` until pushes give it elements); otherwise it stays `never[]`.
    if matches!(initializer, ParsedExpression::ArrayLiteral { elements, .. } if elements.is_empty())
    {
        return if auto_array || !surge_ts_types::strict_null_checks() {
            Type::Array(Box::new(Type::Any))
        } else {
            ty.clone()
        };
    }
    // Without `strictNullChecks`, `undefined` and `null` are widening types:
    // a declaration initialized with one is `any` (`getWidenedType`).
    if !surge_ts_types::strict_null_checks() {
        match ty {
            Type::Undefined => return Type::Any,
            Type::Array(element) if **element == Type::Undefined => {
                return Type::Array(Box::new(Type::Any));
            }
            _ => {}
        }
    }
    // tsc widens only fresh literal types, and an assertion (`as const`,
    // `as "a"`, `<T>x`) yields its regular type, so `let s = "a" as const`
    // stays `"a"`.
    let is_assertion = matches!(
        initializer,
        ParsedExpression::ConstAssertion { .. } | ParsedExpression::TypeAssertion { .. }
    );
    // A mutable binding initialized to `null`/`undefined` is auto-typed: it
    // evolves with its assignments, which surge approximates as `any`.
    if matches!(symbol_kind, SymbolKind::Let | SymbolKind::Var)
        && matches!(ty, Type::Null | Type::Undefined)
    {
        return Type::Any;
    }
    let ty = &widen_nullable_type(ty);
    // A *bare* literal type is widened however it was reached: freshness
    // survives a `const` read (`const a = "x"; let b = a` is `string`), and
    // surge does not track it on the type itself. A union of literals is not
    // widened unless it was written here, which is what keeps
    // `let status = state.status` at its declared union.
    let widens_as_literal = matches!(
        ty,
        Type::StringLiteral(_) | Type::NumberLiteral(_) | Type::BooleanLiteral(_)
    );
    let widened = if matches!(symbol_kind, SymbolKind::Let | SymbolKind::Var)
        && !is_assertion
        && (widens_as_literal || initializer_type_is_fresh(initializer))
    {
        // tsc deep-widens `let`/`var` initializers, so object properties and
        // array/union members widen too (e.g. `let o = { a: 1 }` -> `{ a: number }`),
        // not just a top-level primitive literal.
        widen_type(ty)
    } else if matches!(initializer, ParsedExpression::ObjectLiteral { .. }) {
        widen_object_literal_members(initializer, ty)
    } else {
        ty.clone()
    };
    if writes_object_literal_union(initializer) {
        normalize_object_literal_union(&widened)
    } else {
        widened
    }
}

/// Whether the initializer writes object literals that widen together: the
/// branches of a conditional or logical expression, or the elements of an
/// array literal (read whole or through an index).
fn writes_object_literal_union(initializer: &ParsedExpression) -> bool {
    fn written(expression: &ParsedExpression) -> usize {
        match expression {
            ParsedExpression::ObjectLiteral { .. } => 1,
            ParsedExpression::Conditional {
                when_true,
                when_false,
                ..
            } => written(when_true) + written(when_false),
            ParsedExpression::Logical { left, right, .. }
            | ParsedExpression::NullishCoalescing { left, right, .. } => {
                written(left) + written(right)
            }
            ParsedExpression::ArrayLiteral { elements, .. } => elements
                .iter()
                .filter(|element| !element.spread)
                .map(|element| written(&element.expression))
                .sum(),
            ParsedExpression::ElementAccess { object, .. } => written(object),
            _ => 0,
        }
    }
    written(initializer) >= 2
}

/// tsc's `getWidenedTypeOfObjectLiteral`: object literals widened together are
/// normalized, each gaining the names its siblings write as `name?: undefined`,
/// so `(c ? { a: 1 } : { a: 1, b: "x" }).b` reads `string | undefined` instead
/// of being a missing property. The members keep the types they were written
/// with: `type: "ok" as const` stays the discriminant it is.
fn normalize_object_literal_union(ty: &Type) -> Type {
    let is_written_literal = |member: &Type| {
        matches!(member, Type::Object(object)
            if object.alias_name.is_none()
                && !object.without_inferable_index
                && !object.synthetic_open_index
                && object.string_index_type.is_none()
                && object.number_index_type.is_none()
                && object.call_signature().is_none()
                && object.construct_signature().is_none())
    };
    match ty {
        Type::Array(element) => Type::Array(Box::new(normalize_object_literal_union(element))),
        Type::Union(union) => {
            let members = union.types();
            let mut names: Vec<std::sync::Arc<str>> = Vec::new();
            let mut literals = 0;
            for member in members.iter().filter(|member| is_written_literal(member)) {
                literals += 1;
                if let Type::Object(object) = member {
                    for name in object.properties.keys() {
                        if !names.contains(name) {
                            names.push(name.clone());
                        }
                    }
                }
            }
            if literals < 2 {
                return ty.clone();
            }
            let normalized = members
                .iter()
                .map(|member| match member {
                    Type::Object(object) if is_written_literal(member) => {
                        let mut properties = (*object.properties).clone();
                        for name in &names {
                            if !properties.contains_key(name) {
                                properties.insert(
                                    name.clone(),
                                    surge_ts_types::ObjectProperty::optional(Type::Undefined),
                                );
                            }
                        }
                        Type::Object(crate::metrics::alloc_object_type(properties, None))
                    }
                    other => other.clone(),
                })
                .collect();
            surge_ts_types::union_type(normalized)
        }
        _ => ty.clone(),
    }
}

/// tsc's `getWidenedType` without `strictNullChecks`: `null` and `undefined`
/// widen to `any` wherever a declaration's type is inferred, members included.
pub(crate) fn widen_nullable_type(ty: &Type) -> Type {
    if surge_ts_types::strict_null_checks() {
        return ty.clone();
    }
    match ty {
        Type::Null | Type::Undefined => Type::Any,
        Type::Array(element) => Type::Array(Box::new(widen_nullable_type(element))),
        Type::Tuple(elements) => Type::Tuple(elements.iter().map(widen_nullable_type).collect()),
        Type::Object(object)
            if object.alias_name.is_none()
                && object
                    .properties
                    .values()
                    .any(|property| matches!(property.ty, Type::Null | Type::Undefined)) =>
        {
            let mut properties = (*object.properties).clone();
            for property in properties.values_mut() {
                property.ty = widen_nullable_type(&property.ty);
            }
            let mut widened = crate::metrics::alloc_object_type(
                properties,
                object.string_index_type.as_deref().cloned(),
            );
            if object.synthetic_open_index {
                widened = widened.with_open_index_marker();
            }
            if let Some(call_signature) = object.call_signature() {
                widened = widened.with_call_signature(call_signature.clone());
            }
            if let Some(construct_signature) = object.construct_signature() {
                widened = widened.with_construct_signature(construct_signature.clone());
            }
            Type::Object(widened)
        }
        _ => ty.clone(),
    }
}

/// Whether the initializer's type is *fresh* — written as a literal here — which
/// is the only kind tsc widens (`getWidenedLiteralType`). A literal type read
/// from somewhere else (a property of a union-typed object, a call's declared
/// return) is regular, so `let status = state.status` keeps the union instead
/// of widening to `string`.
fn initializer_type_is_fresh(initializer: &ParsedExpression) -> bool {
    match initializer {
        ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BigIntLiteral(_)
        | ParsedExpression::BooleanLiteral(_)
        | ParsedExpression::NullLiteral
        | ParsedExpression::UndefinedLiteral
        | ParsedExpression::ObjectLiteral { .. }
        | ParsedExpression::ArrayLiteral { .. }
        | ParsedExpression::TemplateLiteral { .. }
        | ParsedExpression::Unary { .. } => true,
        ParsedExpression::Conditional {
            when_true,
            when_false,
            ..
        } => initializer_type_is_fresh(when_true) && initializer_type_is_fresh(when_false),
        ParsedExpression::Logical { left, right, .. }
        | ParsedExpression::NullishCoalescing { left, right, .. } => {
            initializer_type_is_fresh(left) && initializer_type_is_fresh(right)
        }
        _ => false,
    }
}

/// A property initializer is a mutable location, so tsc widens its fresh
/// literal type even under `const` (`checkExpressionForMutableLocation`):
/// `const o = { a: 1 }` is `{ a: number }`. Only members written as literals
/// are widened — surge does not track freshness, and an identifier or an
/// assertion may carry a regular literal type that tsc keeps.
fn widen_object_literal_members(initializer: &ParsedExpression, ty: &Type) -> Type {
    let (ParsedExpression::ObjectLiteral { properties, .. }, Type::Object(object)) =
        (initializer, ty)
    else {
        return ty.clone();
    };
    if object.alias_name.is_some() {
        return ty.clone();
    }
    let mut widened_properties = (*object.properties).clone();
    let mut changed = false;
    for property in properties {
        if property.is_spread || property.is_method || property.is_accessor {
            continue;
        }
        let Some(member) = widened_properties.get_mut(property.name.as_str()) else {
            continue;
        };
        let widened = match &property.value {
            ParsedExpression::StringLiteral(_)
            | ParsedExpression::NumberLiteral(_)
            | ParsedExpression::BooleanLiteral(_)
            | ParsedExpression::TemplateLiteral { .. } => widen_type(&member.ty),
            nested @ ParsedExpression::ObjectLiteral { .. } => {
                widen_object_literal_members(nested, &member.ty)
            }
            _ => continue,
        };
        if widened != member.ty {
            member.ty = widened;
            changed = true;
        }
    }
    if !changed {
        return ty.clone();
    }
    let mut rebuilt = crate::metrics::alloc_object_type(
        widened_properties,
        object.string_index_type.as_deref().cloned(),
    );
    if object.synthetic_open_index {
        rebuilt = rebuilt.with_open_index_marker();
    }
    if let Some(call_signature) = object.call_signature() {
        rebuilt = rebuilt.with_call_signature(call_signature.clone());
    }
    if let Some(construct_signature) = object.construct_signature() {
        rebuilt = rebuilt.with_construct_signature(construct_signature.clone());
    }
    Type::Object(rebuilt)
}

/// Whether `ty` carries the `unknown` *degradation sentinel* anywhere in its
/// shape. The genuine `unknown` keyword ([`Type::GenuineUnknown`]) is
/// deliberately NOT matched: a declared `{ input?: unknown }` member is a real,
/// checkable type in tsc, and suppressing the assignability check for it hides
/// genuine TS2322s. Only surge's could-not-model sentinel warrants no-cascade.
fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::TypeParameter(parameter) => !crate::checks::assign::is_bound_type_parameter(parameter),
        Type::Array(element) => type_contains_unknown(element),
        Type::Tuple(elements) => elements.iter().any(type_contains_unknown),
        // A resolved generic member signature carries the sentinel where its
        // own type parameters (and a self-reference instantiated with them)
        // were erased; that is a bound name, not a gap — see the same arm in
        // `checks::function::body::contains_unknown`.
        Type::Function(function) if function.type_parameter_head().is_some() => false,
        Type::Function(function) => {
            !crate::checks::call::is_generic_signature(function)
                && crate::checks::assign::with_signature_type_parameters(function, || {
                    function.parameters().iter().any(type_contains_unknown)
                        || type_contains_unknown(function.return_type())
                })
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

/// The source an array-pattern element reads from: the object of the
/// `source[index]` access the pattern lowers each element to, beneath a
/// default's `??`.
fn array_pattern_source(initializer: &ParsedExpression) -> Option<ParsedExpression> {
    match initializer {
        ParsedExpression::NullishCoalescing { left, .. } => array_pattern_source(left),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            ..
        } if matches!(index.as_ref(), ParsedExpression::NumberLiteral(_)) => {
            Some(ParsedExpression::Identifier {
                name: object_name.clone(),
                span: *object_span,
            })
        }
        ParsedExpression::ElementAccess { object, index, .. }
            if matches!(index.as_ref(), ParsedExpression::NumberLiteral(_)) =>
        {
            Some(object.as_ref().clone())
        }
        _ => None,
    }
}

fn is_symbol_constructor_call(expression: &ParsedExpression) -> bool {
    match expression {
        ParsedExpression::Call { callee_name, .. } => callee_name == "Symbol",
        ParsedExpression::PropertyCall {
            object,
            property_name,
            ..
        } => {
            property_name == "for"
                && matches!(object.as_ref(), ParsedExpression::Identifier { name, .. } if name == "Symbol")
        }
        _ => false,
    }
}
