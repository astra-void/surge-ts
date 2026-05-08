use oxc_ast::ast::{
    BindingPattern, FormalParameter, PropertyKey, TSInterfaceDeclaration, TSInterfaceHeritage,
    TSMethodSignature, TSMethodSignatureKind, TSSignature,
};

use crate::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedInterfaceDeclaration,
    ParsedInterfaceMember, ParsedType,
};

use super::spans::text_span_from_oxc_span;
use super::types::parse_type_property_signature;
use super::types::{parse_type_annotation, parse_type_parameters};

pub(crate) fn parse_interface_declaration(
    declaration: &TSInterfaceDeclaration<'_>,
) -> Option<ParsedInterfaceDeclaration> {
    let members = declaration
        .body
        .body
        .iter()
        .filter_map(parse_interface_member)
        .collect();

    Some(ParsedInterfaceDeclaration {
        is_declare: declaration.declare,
        name: declaration.id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(declaration.id.span)),
        type_parameters: parse_type_parameters(declaration.type_parameters.as_deref()),
        extends: declaration
            .extends
            .iter()
            .filter_map(parse_interface_heritage)
            .collect(),
        members,
    })
}

fn parse_interface_heritage(heritage: &TSInterfaceHeritage<'_>) -> Option<crate::ParsedNamedType> {
    let oxc_ast::ast::Expression::Identifier(identifier) = &heritage.expression else {
        return None;
    };

    let type_arguments = heritage
        .type_arguments
        .as_deref()
        .and_then(super::types::parse_type_arguments)
        .unwrap_or_default();

    Some(crate::ParsedNamedType {
        name: identifier.name.to_string(),
        span: Some(text_span_from_oxc_span(identifier.span)),
        type_arguments,
    })
}

fn parse_interface_member(member: &TSSignature<'_>) -> Option<ParsedInterfaceMember> {
    match member {
        TSSignature::TSPropertySignature(property_signature) => {
            let property = parse_type_property_signature(property_signature)?;

            Some(ParsedInterfaceMember {
                name: property.name,
                name_span: property.name_span,
                optional: property.optional,
                ty: property.ty,
            })
        }
        TSSignature::TSMethodSignature(method_signature) => {
            parse_interface_method_signature(method_signature)
        }
        _ => None,
    }
}

fn parse_interface_method_signature(
    method_signature: &TSMethodSignature<'_>,
) -> Option<ParsedInterfaceMember> {
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
        .map(parse_method_parameter)
        .collect::<Option<Vec<_>>>()?;

    let return_type = method_signature
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation.as_ref()))?;

    Some(ParsedInterfaceMember {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        optional: method_signature.optional,
        ty: ParsedType::Function(ParsedFunctionType {
            parameters,
            return_type: Box::new(return_type),
            type_parameters: parse_type_parameters(method_signature.type_parameters.as_deref()),
        }),
    })
}

fn parse_method_parameter(parameter: &FormalParameter<'_>) -> Option<ParsedFunctionTypeParameter> {
    let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
        return None;
    };

    let ty = parameter
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedFunctionTypeParameter {
        name: Some(binding.name.to_string()),
        name_span: Some(text_span_from_oxc_span(binding.span)),
        ty,
        optional: parameter.optional || parameter.initializer.is_some(),
    })
}
