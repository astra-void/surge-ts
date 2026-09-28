//! The `with { … }` clauses of a file's import and export declarations, which
//! tsc's `checkImportAttributes` relates to the global `ImportAttributes`.

use oxc_allocator::Allocator;
use oxc_ast::ast::{ImportAttributeKey, ImportOrExportKind, Program, Statement, WithClause};
use oxc_parser::Parser;
use oxc_span::SourceType;

use super::spans::text_span_from_oxc_span;
use crate::{ParsedExpression, ParsedImportAttributes};

pub(crate) fn collect_import_attributes(program: &Program<'_>, source_text: &str) -> Vec<ParsedImportAttributes> {
    let mut clauses = Vec::new();
    for statement in &program.body {
        let (clause, kind) = match statement {
            Statement::ImportDeclaration(declaration) => (declaration.with_clause.as_deref(), declaration.import_kind),
            Statement::ExportAllDeclaration(declaration) => (declaration.with_clause.as_deref(), declaration.export_kind),
            Statement::ExportNamedDeclaration(declaration) => (declaration.with_clause.as_deref(), declaration.export_kind),
            _ => continue,
        };
        if let Some(clause) = clause {
            clauses.push(lower_clause(clause, matches!(kind, ImportOrExportKind::Type), source_text));
        }
    }
    clauses
}

fn lower_clause(clause: &WithClause<'_>, type_only: bool, source_text: &str) -> ParsedImportAttributes {
    let mut span = text_span_from_oxc_span(clause.span);
    // The clause's span starts at its `{`; tsc's node starts at the keyword.
    if let Some(before) = source_text.get(..span.start) {
        let trimmed = before.trim_end();
        for keyword in ["with", "assert"] {
            if let Some(start) = trimmed.strip_suffix(keyword) {
                span.start = start.len();
                break;
            }
        }
    }
    let attributes = clause
        .with_entries
        .iter()
        .map(|attribute| {
            let name = match &attribute.key {
                ImportAttributeKey::Identifier(identifier) => identifier.name.to_string(),
                ImportAttributeKey::StringLiteral(literal) => literal.value.to_string(),
            };
            (name, attribute_value(&attribute.value, source_text))
        })
        .collect();
    ParsedImportAttributes {
        span,
        attributes,
        type_only,
    }
}

/// The recovering parse keeps a value that is no string literal (TS2858) as an
/// empty literal over the expression's span; tsc still checks that expression,
/// so it is parsed again from the text.
fn attribute_value(value: &oxc_ast::ast::StringLiteral<'_>, source_text: &str) -> ParsedExpression {
    let text = source_text
        .get(value.span.start as usize..value.span.end as usize)
        .unwrap_or_default();
    if text.starts_with(['"', '\'']) {
        return ParsedExpression::StringLiteral(value.value.to_string());
    }
    let allocator = Allocator::default();
    match Parser::new(&allocator, text, SourceType::ts()).parse_expression() {
        Ok(expression) => super::expressions::parse_expression(&expression).0,
        Err(_) => ParsedExpression::Unknown,
    }
}
