use oxc_ast::ast::{
    PropertyKey, TSIndexedAccessType, TSLiteral, TSLiteralType, TSMappedType,
    TSMappedTypeModifierOperator, TSMethodSignature, TSMethodSignatureKind, TSPropertySignature,
    TSSignature, TSTupleElement, TSTupleType, TSType, TSTypeAliasDeclaration, TSTypeLiteral,
    TSTypeName, TSTypeOperator, TSTypeOperatorOperator, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQuery, TSTypeQueryExprName,
    TSTypeReference, TSUnionType,
};
use oxc_span::GetSpan;

use crate::{
    ParsedFunctionType, ParsedIndexedAccessType, ParsedMappedType, ParsedNamedType,
    ParsedObjectType, ParsedObjectTypeProperty, ParsedType, ParsedTypeAliasDeclaration,
    ParsedTypeOfType, ParsedTypeParameter,
};

use super::function_types::{parse_function_type, parse_function_type_parameter};
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
        TSType::TSNullKeyword(_) => Some(ParsedType::Undefined),
        TSType::TSObjectKeyword(_) => Some(ParsedType::Unknown),
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
        TSType::TSTypeQuery(type_query) => parse_type_query(type_query),
        TSType::TSTypeOperatorType(type_operator) => parse_type_operator(type_operator),
        TSType::TSIndexedAccessType(indexed_access) => parse_indexed_access_type(indexed_access),
        TSType::TSMappedType(mapped_type) => parse_mapped_type(mapped_type),
        _ => None,
    }
}

fn parse_type_query(type_query: &TSTypeQuery<'_>) -> Option<ParsedType> {
    let name = match &type_query.expr_name {
        TSTypeQueryExprName::IdentifierReference(identifier) => identifier,
        _ => return None, // Fallback for unsupported typeof targets
    };

    Some(ParsedType::TypeOf(ParsedTypeOfType {
        name: name.name.to_string(),
        name_span: Some(text_span_from_oxc_span(name.span)),
    }))
}

fn parse_type_operator(type_operator: &TSTypeOperator<'_>) -> Option<ParsedType> {
    match type_operator.operator {
        TSTypeOperatorOperator::Keyof => {
            parse_type(&type_operator.type_annotation).map(|ty| ParsedType::KeyOf(Box::new(ty)))
        }
        _ => None,
    }
}

fn parse_indexed_access_type(indexed_access: &TSIndexedAccessType<'_>) -> Option<ParsedType> {
    let object_type = parse_type(&indexed_access.object_type)?;
    let index_type = parse_type(&indexed_access.index_type)?;

    Some(ParsedType::IndexedAccess(ParsedIndexedAccessType {
        object_type: Box::new(object_type),
        index_type: Box::new(index_type),
        span: Some(text_span_from_oxc_span(indexed_access.span)),
    }))
}

fn parse_mapped_type(mapped_type: &TSMappedType<'_>) -> Option<ParsedType> {
    if mapped_type.readonly.is_some() || mapped_type.name_type.is_some() {
        return Some(ParsedType::Unknown);
    }

    let optional = match mapped_type.optional {
        Some(TSMappedTypeModifierOperator::True) => true,
        Some(_) => return Some(ParsedType::Unknown), // unsupported +? or -?
        None => false,
    };

    let constraint = parse_type(&mapped_type.constraint)?;
    let value_type = match &mapped_type.type_annotation {
        Some(t) => parse_type(t)?,
        None => ParsedType::Any, // Though typically TS requires a type, fall back to Any or Unknown
    };

    Some(ParsedType::Mapped(ParsedMappedType {
        key_name: mapped_type.key.name.to_string(),
        key_span: Some(text_span_from_oxc_span(mapped_type.key.span)),
        constraint: Box::new(constraint),
        value_type: Box::new(value_type),
        optional,
        span: Some(text_span_from_oxc_span(mapped_type.span)),
    }))
}

fn parse_type_reference(type_reference: &TSTypeReference<'_>) -> Option<ParsedType> {
    let TSTypeName::IdentifierReference(identifier) = &type_reference.type_name else {
        return None;
    };

    let type_arguments = match type_reference.type_arguments.as_deref() {
        Some(type_arguments) => parse_type_arguments(type_arguments)?,
        None => Vec::new(),
    };

    Some(ParsedType::Named(ParsedNamedType {
        name: identifier.name.to_string(),
        span: Some(text_span_from_oxc_span(identifier.span)),
        type_arguments,
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
        let property = match member {
            TSSignature::TSPropertySignature(property_signature) => {
                parse_type_property_signature(property_signature)
            }
            TSSignature::TSMethodSignature(method_signature) => {
                parse_type_method_signature(method_signature)
            }
            _ => return ParsedType::Unknown,
        };

        let Some(property) = property else {
            return ParsedType::Unknown;
        };

        properties.push(property);
    }

    ParsedType::Object(ParsedObjectType { properties })
}

/// Lowers a method signature (`foo(arg: A): R`) into a property whose type is a
/// [`ParsedType::Function`], so method calls reuse the existing function-type property
/// checking. Shared by interface and object-type-literal parsing.
pub(crate) fn parse_type_method_signature(
    method_signature: &TSMethodSignature<'_>,
) -> Option<ParsedObjectTypeProperty> {
    if method_signature.kind != TSMethodSignatureKind::Method || method_signature.computed {
        return None;
    }

    let PropertyKey::StaticIdentifier(key) = &method_signature.key else {
        return None;
    };

    let parameters = method_signature
        .params
        .items
        .iter()
        .map(parse_function_type_parameter)
        .collect::<Option<Vec<_>>>()?;

    let return_type = method_signature
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation.as_ref()))?;

    Some(ParsedObjectTypeProperty {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        ty: ParsedType::Function(ParsedFunctionType {
            parameters,
            return_type: Box::new(return_type),
            type_parameters: parse_type_parameters(method_signature.type_parameters.as_deref()),
        }),
        optional: method_signature.optional,
    })
}

pub(crate) fn parse_type_parameters(
    type_parameters: Option<&TSTypeParameterDeclaration<'_>>,
) -> Vec<ParsedTypeParameter> {
    type_parameters
        .map(|type_parameters| {
            type_parameters
                .params
                .iter()
                .map(parse_type_parameter)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_type_parameter(type_parameter: &TSTypeParameter<'_>) -> ParsedTypeParameter {
    ParsedTypeParameter {
        name: type_parameter.name.name.to_string(),
        name_span: Some(text_span_from_oxc_span(type_parameter.name.span)),
        constraint: type_parameter.constraint.as_ref().and_then(parse_type),
        default_type: type_parameter.default.as_ref().and_then(parse_type),
        span: Some(text_span_from_oxc_span(type_parameter.span)),
    }
}

pub(crate) fn parse_type_arguments(
    type_arguments: &TSTypeParameterInstantiation<'_>,
) -> Option<Vec<ParsedType>> {
    type_arguments.params.iter().map(parse_type).collect()
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
    let ty = parse_type(&declaration.type_annotation)?;

    Some(ParsedTypeAliasDeclaration {
        is_declare: declaration.declare,
        name: declaration.id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(declaration.id.span)),
        type_parameters: parse_type_parameters(declaration.type_parameters.as_deref()),
        ty,
        type_span: Some(text_span_from_oxc_span(declaration.type_annotation.span())),
    })
}
