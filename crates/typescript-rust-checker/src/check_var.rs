use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::ParsedVariableDeclaration;
use typescript_rust_types::{Type, is_assignable_to};

use crate::check_expr::{
    ExpectedTypeDiagnostic, evaluate_expression, evaluate_expression_with_expected_type,
};
use crate::context::{CheckerContext, convert_span};
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::{SymbolInfo, SymbolKind, SymbolTable, map_symbol_kind};

pub(crate) struct VariableCheckOptions {
    pub(crate) report_duplicate_let_const: bool,
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
    let declared_type = variable.declared_type.map(map_parsed_type);
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

    let inferred_initializer = variable
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
        .unwrap_or(InferredExpression::Unknown);

    let inferred_symbol_type = match &inferred_initializer {
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
                Some((*inferred_initializer_type).clone())
            } else {
                None
            }
        }
        InferredExpression::UnresolvedIdentifier { .. } => None,
        InferredExpression::MissingProperty { .. } => None,
        InferredExpression::Unknown => None,
    };

    declared_type.or(inferred_symbol_type).map(|ty| {
        let symbol = SymbolInfo {
            ty,
            kind: symbol_kind,
        };

        symbols.insert(variable.name, symbol.clone());
        symbol
    })
}
