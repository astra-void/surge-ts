use oxc_ast::ast::{
    PropertyKey, TSLiteral, TSLiteralType, TSPropertySignature, TSSignature, TSTupleElement,
    TSTupleType, TSType, TSTypeAliasDeclaration, TSTypeLiteral, TSTypeName, TSTypeReference,
    TSUnionType,
};
use oxc_span::GetSpan;

use crate::{
    ParsedNamedType, ParsedObjectType, ParsedObjectTypeProperty, ParsedType,
    ParsedTypeAliasDeclaration,
};

use super::function_types::parse_function_type;
use super::spans::text_span_from_oxc_span;

pub(crate) fn parse_type_annotation(
    type_annotation: &oxc_ast::ast::TSTypeAnnotation<'_>,
) -> Option<ParsedType> {
    parse_type(&type_annotation.type_annotation)
}

pub(crate) fn parse_type(type_annotation: &TSType<'_>) -> Option<ParsedType> {
    match type_annotation {
        TSType::TSStringKeyword(_) => Some(ParsedType::String),
        TSType::TSNumberKeyword(_) => Some(ParsedType::Number),
        TSType::TSBooleanKeyword(_) => Some(ParsedType::Boolean),
        TSType::TSUndefinedKeyword(_) => Some(ParsedType::Undefined),
        TSType::TSVoidKeyword(_) => Some(ParsedType::Void),
        TSType::TSAnyKeyword(_) => Some(ParsedType::Any),
        TSType::TSUnknownKeyword(_) => Some(ParsedType::Unknown),
        TSType::TSLiteralType(literal_type) => Some(parse_literal_type(literal_type)),
        TSType::TSTypeLiteral(type_literal) => Some(parse_type_literal(type_literal)),
        TSType::TSArrayType(array_type) => {
            parse_type(&array_type.element_type).map(|ty| ParsedType::Array(Box::new(ty)))
        }
        TSType::TSTupleType(tuple_type) => parse_tuple_type(tuple_type),
        TSType::TSFunctionType(function_type) => {
            parse_function_type(function_type).map(ParsedType::Function)
        }
        TSType::TSParenthesizedType(parenthesized_type) => {
            parse_type(&parenthesized_type.type_annotation)
        }
        TSType::TSUnionType(union_type) => Some(parse_union_type(union_type)),
        TSType::TSTypeReference(type_reference) => parse_type_reference(type_reference),
        _ => None,
    }
}

fn parse_type_reference(type_reference: &TSTypeReference<'_>) -> Option<ParsedType> {
    if type_reference.type_arguments.is_some() {
        return None;
    }

    let TSTypeName::IdentifierReference(identifier) = &type_reference.type_name else {
        return None;
    };

    Some(ParsedType::Named(ParsedNamedType {
        name: identifier.name.to_string(),
        span: Some(text_span_from_oxc_span(identifier.span)),
    }))
}

fn parse_literal_type(literal_type: &TSLiteralType<'_>) -> ParsedType {
    match &literal_type.literal {
        TSLiteral::StringLiteral(string_literal) => {
            ParsedType::StringLiteral(string_literal.value.to_string())
        }
        TSLiteral::NumericLiteral(numeric_literal) => {
            ParsedType::NumberLiteral(numeric_literal.value.to_string())
        }
        TSLiteral::BooleanLiteral(boolean_literal) => {
            ParsedType::BooleanLiteral(boolean_literal.value)
        }
        TSLiteral::UnaryExpression(_)
        | TSLiteral::BigIntLiteral(_)
        | TSLiteral::TemplateLiteral(_) => ParsedType::Unknown,
    }
}

fn parse_union_type(union_type: &TSUnionType<'_>) -> ParsedType {
    let mut types = Vec::new();

    for ty in &union_type.types {
        let Some(parsed_type) = parse_type(ty) else {
            return ParsedType::Unknown;
        };

        types.push(parsed_type);
    }

    ParsedType::Union(types)
}

fn parse_tuple_type(tuple_type: &TSTupleType<'_>) -> Option<ParsedType> {
    let mut elements = Vec::new();

    for element in &tuple_type.element_types {
        match element {
            TSTupleElement::TSNamedTupleMember(_)
            | TSTupleElement::TSOptionalType(_)
            | TSTupleElement::TSRestType(_) => return None,
            _ => {
                let Some(parsed_element) = parse_type(element.as_ts_type()?) else {
                    return None;
                };

                elements.push(parsed_element);
            }
        }
    }

    Some(ParsedType::Tuple(elements))
}

fn parse_type_literal(type_literal: &TSTypeLiteral<'_>) -> ParsedType {
    let mut properties = Vec::new();

    for member in &type_literal.members {
        let TSSignature::TSPropertySignature(property_signature) = member else {
            return ParsedType::Unknown;
        };

        let Some(property) = parse_type_property_signature(property_signature) else {
            return ParsedType::Unknown;
        };

        properties.push(property);
    }

    ParsedType::Object(ParsedObjectType { properties })
}

pub(crate) fn parse_type_property_signature(
    property_signature: &TSPropertySignature<'_>,
) -> Option<ParsedObjectTypeProperty> {
    if property_signature.readonly || property_signature.computed {
        return None;
    }

    let PropertyKey::StaticIdentifier(key) = &property_signature.key else {
        return None;
    };

    let type_annotation = property_signature
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedObjectTypeProperty {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        ty: type_annotation,
        optional: property_signature.optional,
    })
}

pub(crate) fn parse_type_alias_declaration(
    declaration: &TSTypeAliasDeclaration<'_>,
) -> Option<ParsedTypeAliasDeclaration> {
    if declaration.type_parameters.is_some() {
        return None;
    }

    let ty = parse_type(&declaration.type_annotation)?;

    Some(ParsedTypeAliasDeclaration {
        name: declaration.id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(declaration.id.span)),
        ty,
        type_span: Some(text_span_from_oxc_span(declaration.type_annotation.span())),
    })
}
