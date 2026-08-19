//! `enum` declarations, lowered to the constructs the checker already models:
//! a type alias holding the union of the member types, plus a same-named
//! ambient `const` whose object type carries one property per member. Enum
//! types are nominal in tsc; the literal-union approximation keeps every member
//! read (`Color.Red`), value use (`z.enum(Color)`), and type position resolving
//! instead of cascading `TS2304`, at the cost of accepting a bare literal where
//! tsc would require the enum member.

use oxc_ast::ast::{Expression, TSEnumDeclaration, TSEnumMemberName};

use crate::{
    ParsedFunctionBodyStatement, ParsedObjectType, ParsedObjectTypeProperty, ParsedStatement,
    ParsedType, ParsedTypeAliasDeclaration, ParsedVariableDeclaration, ParsedVariableKind,
};

use super::text_span_from_oxc_span;

pub(crate) fn parse_enum_declaration(declaration: &TSEnumDeclaration<'_>) -> Vec<ParsedStatement> {
    let (type_alias, value) = lower_enum_declaration(declaration);
    vec![
        ParsedStatement::TypeAliasDeclaration(Box::new(type_alias)),
        ParsedStatement::VariableDeclaration(Box::new(value)),
    ]
}

pub(crate) fn parse_enum_declaration_as_function_body(
    declaration: &TSEnumDeclaration<'_>,
) -> Vec<ParsedFunctionBodyStatement> {
    let (type_alias, value) = lower_enum_declaration(declaration);
    vec![
        ParsedFunctionBodyStatement::TypeAlias(Box::new(type_alias)),
        ParsedFunctionBodyStatement::VariableDeclaration(Box::new(value)),
    ]
}

fn lower_enum_declaration(
    declaration: &TSEnumDeclaration<'_>,
) -> (ParsedTypeAliasDeclaration, ParsedVariableDeclaration) {
    let name_span = Some(text_span_from_oxc_span(declaration.id.span));
    let mut next_auto_value: f64 = 0.0;
    let mut properties = Vec::with_capacity(declaration.body.members.len());
    let mut member_types = Vec::with_capacity(declaration.body.members.len());

    for member in &declaration.body.members {
        let Some(member_name) = enum_member_name(&member.id) else {
            continue;
        };
        let member_type = match member.initializer.as_ref() {
            Some(initializer) => match constant_member_type(initializer) {
                Some(ParsedType::NumberLiteral(value)) => {
                    if let Ok(parsed) = value.parse::<f64>() {
                        next_auto_value = parsed + 1.0;
                    }
                    ParsedType::NumberLiteral(value)
                }
                Some(other) => other,
                // A computed member (`A = f()`, `B = A | C`) is numeric in TS but
                // its value is not statically known here; widening keeps the
                // member readable without inventing a wrong literal. Auto values
                // after it are equally unknown, so they widen too.
                None => {
                    next_auto_value = f64::NAN;
                    ParsedType::Number
                }
            },
            None => {
                if next_auto_value.is_nan() {
                    ParsedType::Number
                } else {
                    let value = format_auto_value(next_auto_value);
                    next_auto_value += 1.0;
                    ParsedType::NumberLiteral(value)
                }
            }
        };

        properties.push(ParsedObjectTypeProperty {
            name: member_name,
            name_span: Some(text_span_from_oxc_span(member.span)),
            ty: member_type.clone(),
            optional: false,
            is_method: false,
        });
        member_types.push(member_type);
    }

    let enum_type = match member_types.len() {
        0 => ParsedType::Never,
        1 => member_types.pop().expect("one member type"),
        _ => ParsedType::Union(std::sync::Arc::new(member_types)),
    };

    (
        ParsedTypeAliasDeclaration {
            is_declare: declaration.declare,
            name: declaration.id.name.to_string(),
            name_span,
            type_parameters: Vec::new(),
            ty: enum_type,
            type_span: name_span,
        },
        ParsedVariableDeclaration {
            // The object side has no written initializer to check, and an `enum`
            // is never subject to the initializer-inference paths.
            is_declare: true,
            from_binding_pattern: false,
            has_definite_assertion: false,
            kind: ParsedVariableKind::Const,
            name: declaration.id.name.to_string(),
            name_span,
            declared_type: Some(ParsedType::Object(ParsedObjectType {
                properties,
                string_index_type: None,
                call_signature: None,
            })),
            initializer: None,
            initializer_span: None,
        },
    )
}

fn enum_member_name(name: &TSEnumMemberName<'_>) -> Option<String> {
    match name {
        TSEnumMemberName::Identifier(identifier) => Some(identifier.name.to_string()),
        TSEnumMemberName::String(literal) | TSEnumMemberName::ComputedString(literal) => {
            Some(literal.value.to_string())
        }
        TSEnumMemberName::ComputedTemplateString(_) => None,
    }
}

fn constant_member_type(initializer: &Expression<'_>) -> Option<ParsedType> {
    match initializer {
        Expression::StringLiteral(literal) => {
            Some(ParsedType::StringLiteral(literal.value.to_string()))
        }
        Expression::NumericLiteral(literal) => {
            Some(ParsedType::NumberLiteral(format_auto_value(literal.value)))
        }
        Expression::UnaryExpression(unary)
            if unary.operator == oxc_syntax::operator::UnaryOperator::UnaryNegation =>
        {
            match constant_member_type(&unary.argument)? {
                ParsedType::NumberLiteral(value) => {
                    Some(ParsedType::NumberLiteral(format!("-{value}")))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn format_auto_value(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}
