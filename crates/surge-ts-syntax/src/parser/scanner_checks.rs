//! Scanner and parser errors tsc raises that oxc does not: a legacy octal
//! literal (TS1121) and a decimal with a leading zero (TS1489) are rejected by
//! tsc's `scanNumber` in every file, strict or not, and type arguments on
//! `super` (TS2754) and a `super` followed by anything but an argument list or
//! member access (TS1034) by `parseSuperExpression`, and a type that is only a
//! `?` (TS1110) by `parseJSDocNullableType`. Like any parse error they make the
//! program report its syntactic diagnostics alone.

use oxc_ast::ast::{
    BinaryExpression, CallExpression, ComputedMemberExpression, Expression, JSDocUnknownType,
    NewExpression, NumericLiteral, Program, PrivateFieldExpression, StaticMemberExpression, Super,
    UnaryExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

use crate::{ParserError, TextSpan};

pub(crate) fn collect_missing_parser_errors(program: &Program<'_>, source_text: &str) -> Vec<ParserError> {
    let mut collector = Collector {
        source_text,
        after_minus: Vec::new(),
        followed_super: Vec::new(),
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
    /// `super`s followed by an argument list or member access. A `?.` is
    /// neither to `parseSuperExpression`, which looks only at the next token.
    followed_super: Vec<u32>,
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

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if let Expression::Super(callee) = &new.callee {
            self.followed_super.push(callee.span.start);
        }
        walk::walk_new_expression(self, new);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if let Expression::Super(object) = &member.object
            && !member.optional
        {
            self.followed_super.push(object.span.start);
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::Super(object) = &member.object
            && !member.optional
        {
            self.followed_super.push(object.span.start);
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_private_field_expression(&mut self, member: &PrivateFieldExpression<'a>) {
        if let Expression::Super(object) = &member.object
            && !member.optional
        {
            self.followed_super.push(object.span.start);
        }
        walk::walk_private_field_expression(self, member);
    }

    fn visit_super(&mut self, keyword: &Super) {
        if self.followed_super.contains(&keyword.span.start) {
            return;
        }
        // Reported on the token after the keyword (`parseErrorAtCurrentToken`).
        let after = keyword.span.end as usize;
        let start = after
            + self.source_text[after..]
                .char_indices()
                .find(|(_, c)| !c.is_whitespace())
                .map_or(self.source_text.len() - after, |(offset, _)| offset);
        let end = super::grammar_context::first_token_end(self.source_text, start);
        self.push(1034, "'super' must be followed by an argument list or member access.".to_string(), start, end);
    }

    /// tsc parses a `?` in type position as a nullable type and then requires
    /// the type (`parseJSDocNullableType`); oxc reads a lone `?` before `,`,
    /// `)`, `>`, `=`, `|` or `}` as JSDoc's unknown type. tsc reports the
    /// missing type at that next token.
    fn visit_js_doc_unknown_type(&mut self, it: &JSDocUnknownType) {
        let after = it.span.end as usize;
        let start = after
            + self.source_text[after..]
                .char_indices()
                .find(|(_, c)| !c.is_whitespace())
                .map_or(self.source_text.len() - after, |(offset, _)| offset);
        let end = (start + 1).min(self.source_text.len()).max(start);
        self.push(1110, "Type expected.".to_string(), start, end);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Super(callee) = &call.callee
            && !call.optional
        {
            self.followed_super.push(callee.span.start);
        }
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
