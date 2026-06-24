use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::ParsedVariableDeclaration;
use surge_ts_types::{Type, is_assignable_to};

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

    let inferred_initializer = if options.check_initializer {
        variable
            .initializer
            .as_ref()
            .map(|initializer| {
                if let Some(ref declared_type) = declared_type {
                    evaluate_expression_with_expected_type_anchored(
                        initializer,
                        variable.initializer_span,
                        variable.name_span,
                        Some(declared_type),
                        ExpectedTypeDiagnostic::TypeNotAssignable,
                        symbols,
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

    let mut inferred_symbol_type = match &inferred_initializer {
        InferredExpression::Known(inferred_initializer_type) => {
            if let Some(ref declared_type) = declared_type {
                if !inferred_initializer_type.is_unknown()
                    && !type_contains_unknown(declared_type)
                    && !type_contains_unknown(inferred_initializer_type)
                    && !is_assignable_to(inferred_initializer_type, declared_type)
                {
                    let inferred_type_name =
                        source_display_name(inferred_initializer_type, declared_type);
                    let declared_type_name = declared_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &inferred_type_name,
                        &declared_type_name,
                        ctx.file_name.clone(),
                    );

                    let target_span = variable.name_span.or(variable.initializer_span);
                    let diagnostic = match target_span {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    };

                    ctx.push(diagnostic);
                }
            }

            if declared_type.is_none() && !inferred_initializer_type.is_unknown() {
                Some(widen_implicit_variable_initializer_type(
                    symbol_kind,
                    inferred_initializer_type,
                ))
            } else {
                declared_type.clone().or(Some(Type::Unknown))
            }
        }
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => declared_type.clone().or(Some(Type::Unknown)),
    };

    if declared_type.is_none() && variable.initializer.is_none() {
        inferred_symbol_type = Some(Type::Any);
    }

    declared_type.or(inferred_symbol_type).map(|ty| {
        Arc::new(SymbolInfo {
            ty,
            kind: symbol_kind,
            function_signature: None,
        })
    })
}

pub(crate) fn widen_implicit_variable_initializer_type(symbol_kind: SymbolKind, ty: &Type) -> Type {
    if matches!(symbol_kind, SymbolKind::Let | SymbolKind::Var) {
        // tsc deep-widens `let`/`var` initializers, so object properties and
        // array/union members widen too (e.g. `let o = { a: 1 }` -> `{ a: number }`),
        // not just a top-level primitive literal.
        widen_type(ty)
    } else {
        ty.clone()
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
