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

pub(crate) fn parse_enum_declaration(
    declaration: &TSEnumDeclaration<'_>,
    exported: bool,
) -> Vec<ParsedStatement> {
    let (type_alias, value) = lower_enum_declaration(declaration, exported);
    let mut statements: Vec<ParsedStatement> = member_type_aliases(&type_alias, &value)
        .map(|alias| ParsedStatement::TypeAliasDeclaration(Box::new(alias)))
        .collect();
    statements.push(ParsedStatement::TypeAliasDeclaration(Box::new(type_alias)));
    statements.push(ParsedStatement::VariableDeclaration(Box::new(value)));
    statements
}

/// One alias per member, named `Enum.Member`, so an *enum member type* resolves
/// (`type R = Color.Red`, or a discriminant `interface N { kind: Color.Red }`).
/// Without them the annotation misses and the enclosing declaration degrades,
/// which both loses the diagnostic and makes the expansion uncacheable.
///
/// An enum declared inside a `namespace` is registered qualified
/// (`ts.SyntaxKind.SourceFile`), so a bare `SyntaxKind.SourceFile` written inside
/// that namespace still misses — see the note in
/// `docs/perf/NAMESPACE-INTERFACE-MERGE.md`.
fn member_type_aliases<'a>(
    type_alias: &'a ParsedTypeAliasDeclaration,
    value: &'a ParsedVariableDeclaration,
) -> impl Iterator<Item = ParsedTypeAliasDeclaration> + 'a {
    let members: &'a [ParsedObjectTypeProperty] = match value.declared_type.as_ref() {
        Some(ParsedType::Object(object)) => object.properties.as_slice(),
        _ => &[],
    };
    members.iter().map(move |member| ParsedTypeAliasDeclaration {
        is_declare: type_alias.is_declare,
        name: format!("{}.{}", type_alias.name, member.name),
        name_span: member.name_span,
        type_parameters: Vec::new(),
        ty: member.ty.clone(),
        type_span: member.name_span,
        enum_name: Some(type_alias.name.clone()),
        enum_exported: type_alias.enum_exported,
    })
}

pub(crate) fn parse_enum_declaration_as_function_body(
    declaration: &TSEnumDeclaration<'_>,
) -> Vec<ParsedFunctionBodyStatement> {
    // A body-local enum is never visible outside its file.
    let (type_alias, value) = lower_enum_declaration(declaration, false);
    vec![
        ParsedFunctionBodyStatement::TypeAlias(Box::new(type_alias)),
        ParsedFunctionBodyStatement::VariableDeclaration(Box::new(value)),
    ]
}

fn lower_enum_declaration(
    declaration: &TSEnumDeclaration<'_>,
    exported: bool,
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
            readonly: false,
            write_ty: None,
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
            enum_name: Some(declaration.id.name.to_string()),
            enum_exported: exported,
        },
        ParsedVariableDeclaration {
            // The object side has no written initializer to check, and an `enum`
            // is never subject to the initializer-inference paths.
            is_declare: true,
            from_binding_pattern: false,
            has_definite_assertion: false,
            array_pattern_span: None,
            is_enum_object: true,
            kind: ParsedVariableKind::Const,
            name: declaration.id.name.to_string(),
            name_span,
            declared_type: Some(ParsedType::Object(std::sync::Arc::new(ParsedObjectType {
                properties,
                string_index_type: None,
                number_index_type: None,
                call_signature: None,
            call_signature_overloads: Vec::new(),
                construct_signature: None,
                non_primitive: false,
            }))),
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

/// An `enum` declared more than once in one scope is one enum: tsc merges the
/// bodies. The lowering runs per declaration, so the object sides (and the
/// union alias holding the member types) are folded together here — otherwise
/// one declaration's members silently replace the other's.
pub(crate) fn merge_lowered_enum_declarations(statements: &mut Vec<ParsedStatement>) {
    use std::collections::HashMap;

    let mut objects: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut aliases: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, statement) in statements.iter().enumerate() {
        match peel_exported(statement) {
            ParsedStatement::VariableDeclaration(variable) if variable.is_enum_object => {
                objects.entry(variable.name.as_str()).or_default().push(index);
            }
            ParsedStatement::TypeAliasDeclaration(alias)
                if alias.enum_name.as_deref() == Some(alias.name.as_str()) =>
            {
                aliases.entry(alias.name.as_str()).or_default().push(index);
            }
            _ => {}
        }
    }

    let duplicated: Vec<Vec<usize>> = objects
        .into_values()
        .chain(aliases.into_values())
        .filter(|indices| indices.len() > 1)
        .collect();
    if duplicated.is_empty() {
        return;
    }

    let mut dropped: Vec<usize> = Vec::new();
    for indices in duplicated {
        let (first, rest) = indices.split_first().expect("non-empty group");
        let mut properties: Vec<ParsedObjectTypeProperty> = Vec::new();
        let mut member_types: Vec<ParsedType> = Vec::new();
        for index in rest {
            match peel_exported(&statements[*index]) {
                ParsedStatement::VariableDeclaration(variable) => {
                    if let Some(ParsedType::Object(object)) = variable.declared_type.as_ref() {
                        properties.extend(object.properties.iter().cloned());
                    }
                }
                ParsedStatement::TypeAliasDeclaration(alias) => {
                    push_union_members(&alias.ty, &mut member_types);
                }
                _ => {}
            }
            dropped.push(*index);
        }

        let Some(target) = exported_enum_statement_mut(&mut statements[*first]) else {
            continue;
        };
        match target {
            ParsedStatement::VariableDeclaration(variable) => {
                if let Some(ParsedType::Object(object)) = variable.declared_type.as_mut() {
                    let object = std::sync::Arc::make_mut(object);
                    for property in properties {
                        if !object.properties.iter().any(|kept| kept.name == property.name) {
                            object.properties.push(property);
                        }
                    }
                }
            }
            ParsedStatement::TypeAliasDeclaration(alias) => {
                let mut merged = Vec::new();
                push_union_members(&alias.ty, &mut merged);
                for member in member_types {
                    if !merged.contains(&member) {
                        merged.push(member);
                    }
                }
                alias.ty = match merged.len() {
                    1 => merged.pop().expect("one member"),
                    _ => ParsedType::Union(std::sync::Arc::new(merged)),
                };
            }
            _ => {}
        }
    }

    dropped.sort_unstable();
    let mut index = 0;
    statements.retain(|_| {
        let keep = dropped.binary_search(&index).is_err();
        index += 1;
        keep
    });
}

fn push_union_members(ty: &ParsedType, members: &mut Vec<ParsedType>) {
    match ty {
        ParsedType::Union(union) => members.extend(union.iter().cloned()),
        other => members.push(other.clone()),
    }
}

fn peel_exported(statement: &ParsedStatement) -> &ParsedStatement {
    match statement {
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            crate::ParsedExportDeclaration::Statement { declaration, .. } => {
                peel_exported(declaration.as_ref())
            }
            _ => statement,
        },
        other => other,
    }
}

fn exported_enum_statement_mut(statement: &mut ParsedStatement) -> Option<&mut ParsedStatement> {
    match statement {
        ParsedStatement::ExportDeclaration(export) => match export.as_mut() {
            crate::ParsedExportDeclaration::Statement { declaration, .. } => {
                exported_enum_statement_mut(declaration.as_mut())
            }
            _ => None,
        },
        other => Some(other),
    }
}
