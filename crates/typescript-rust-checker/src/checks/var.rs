use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::ParsedVariableDeclaration;
use typescript_rust_types::{Type, is_assignable_to};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::evaluate_expression;
use crate::context::{CheckerContext, convert_span};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::{SymbolInfo, SymbolKind, SymbolTable, map_symbol_kind};

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
) -> Option<SymbolInfo> {
    // Put symbols back into ctx for map_parsed_type to use
    let temp_symbols = std::mem::take(symbols);
    ctx.set_symbols(temp_symbols);

    let declared_type = variable
        .declared_type
        .map(|declared_type| map_parsed_type(declared_type, ctx));

    // Take them back out
    *symbols = std::mem::take(&mut ctx.symbols);

    let symbol_kind = map_symbol_kind(variable.kind);

    if options.report_duplicate_let_const
        && matches!(symbol_kind, SymbolKind::Let | SymbolKind::Const)
        && symbols.contains_let_or_const(&variable.name)
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
                    evaluate_expression_with_expected_type(
                        initializer,
                        variable.initializer_span,
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
                if *inferred_initializer_type != Type::Unknown
                    && !is_assignable_to(inferred_initializer_type, declared_type)
                {
                    let inferred_type_name = inferred_initializer_type.name();
                    let declared_type_name = declared_type.name();
                    let diagnostic = Diagnostic::ts2322(
                        &inferred_type_name,
                        &declared_type_name,
                        ctx.file_name.clone(),
                    );

                    let diagnostic = match variable.initializer_span {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    };

                    ctx.push(diagnostic);
                }
            }

            if declared_type.is_none() && *inferred_initializer_type != Type::Unknown {
                Some(widen_implicit_variable_initializer_type(
                    symbol_kind,
                    inferred_initializer_type,
                ))
            } else {
                None
            }
        }
        InferredExpression::UnresolvedIdentifier { .. } => None,
        InferredExpression::MissingProperty { .. } => None,
        InferredExpression::Unknown => None,
    };

    if declared_type.is_none()
        && variable.initializer.is_none()
        && ctx.options.no_implicit_any
        && matches!(
            symbol_kind,
            SymbolKind::Var | SymbolKind::Let | SymbolKind::Const
        )
    {
        let diagnostic = Diagnostic::ts7005(&variable.name, "any", ctx.file_name.clone());
        let diagnostic = match variable.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
        inferred_symbol_type = Some(Type::Any);
    }

    declared_type.or(inferred_symbol_type).map(|ty| {
        let symbol = SymbolInfo {
            ty,
            kind: symbol_kind,
        };

        symbols.insert(variable.name, symbol.clone());
        symbol
    })
}

fn widen_implicit_variable_initializer_type(symbol_kind: SymbolKind, ty: &Type) -> Type {
    if matches!(symbol_kind, SymbolKind::Let | SymbolKind::Var) {
        ty.base_primitive().unwrap_or_else(|| ty.clone())
    } else {
        ty.clone()
    }
}
