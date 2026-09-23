//! Scanner and parser errors tsc raises that oxc does not: a legacy octal
//! literal (TS1121) and a decimal with a leading zero (TS1489) are rejected by
//! tsc's `scanNumber` in every file, strict or not, and type arguments on
//! `super` (TS2754) by `parseSuperExpression`. Like any parse error they make
//! the program report its syntactic diagnostics alone.

use oxc_ast::ast::{
    BinaryExpression, CallExpression, Expression, NumericLiteral, Program, UnaryExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::{ParserError, TextSpan};

pub(crate) fn collect_missing_parser_errors(program: &Program<'_>, source_text: &str) -> Vec<ParserError> {
    let mut collector = Collector {
        source_text,
        after_minus: Vec::new(),
        errors: Vec::new(),
    };
    collector.visit_program(program);
    collector.errors
}

struct Collector<'s> {
    source_text: &'s str,
    /// Literals whose previous token is `-`, which the scanner folds into an
    /// octal's suggested spelling and span.
    after_minus: Vec<u32>,
    errors: Vec<ParserError>,
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        if expression.operator == UnaryOperator::UnaryNegation
            && let Expression::NumericLiteral(literal) = &expression.argument
        {
            self.after_minus.push(literal.span.start);
        }
        walk::walk_unary_expression(self, expression);
    }

    fn visit_binary_expression(&mut self, expression: &BinaryExpression<'a>) {
        if expression.operator == BinaryOperator::Subtraction
            && let Expression::NumericLiteral(literal) = &expression.right
        {
            self.after_minus.push(literal.span.start);
        }
        walk::walk_binary_expression(self, expression);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if matches!(call.callee, Expression::Super(_))
            && let Some(type_arguments) = &call.type_arguments
        {
            self.push(
                2754,
                "'super' may not use type arguments.".to_string(),
                type_arguments.span.start as usize,
                type_arguments.span.end as usize,
            );
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_numeric_literal(&mut self, literal: &NumericLiteral<'a>) {
        let start = literal.span.start as usize;
        let Some(raw) = self.source_text.get(start..literal.span.end as usize) else {
            return;
        };
        let bytes = raw.as_bytes();
        if bytes.len() < 2 || bytes[0] != b'0' || !bytes[1].is_ascii_digit() {
            return;
        }
        let digit_count = raw[1..].bytes().take_while(u8::is_ascii_digit).count();
        let digits = &raw[1..1 + digit_count];
        if digits.bytes().any(|digit| digit >= b'8') {
            self.push(1489, "Decimals with leading zeros are not allowed.".to_string(), start, start + raw.len());
            return;
        }
        let Ok(value) = u64::from_str_radix(digits, 8) else {
            return;
        };
        let (start, sign) = if self.after_minus.contains(&literal.span.start) {
            (start.saturating_sub(1), "-")
        } else {
            (start, "")
        };
        let end = literal.span.start as usize + 1 + digit_count;
        self.push(
            1121,
            format!("Octal literals are not allowed. Use the syntax '{sign}0o{value:o}'."),
            start,
            end,
        );
    }
}

impl Collector<'_> {
    fn push(&mut self, code: u32, message: String, start: usize, end: usize) {
        self.errors.push(ParserError {
            code: Some(code),
            message,
            span: Some(TextSpan { start, end }),
            span_text: self.source_text.get(start..end).map(str::to_string),
        });
    }
}
