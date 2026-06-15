use oxc_ast::ast::{TSInterfaceDeclaration, TSInterfaceHeritage, TSSignature};

use crate::{ParsedInterfaceDeclaration, ParsedInterfaceMember};

use super::spans::text_span_from_oxc_span;
use super::types::parse_type_parameters;
use super::types::{
    parse_index_signature_value_type, parse_type_method_signature, parse_type_property_signature,
};

pub(crate) fn parse_interface_declaration(
    declaration: &TSInterfaceDeclaration<'_>,
) -> Option<ParsedInterfaceDeclaration> {
    let members = declaration
        .body
        .body
        .iter()
        .filter_map(parse_interface_member)
        .collect();

    // A string/number index signature (`[key: string]: T`) contributes the
    // object's `string_index_type` rather than a named property. The last one
    // wins (interfaces rarely declare more than one).
    let string_index_type = declaration
        .body
        .body
        .iter()
        .filter_map(|member| match member {
            TSSignature::TSIndexSignature(index_signature) => {
                parse_index_signature_value_type(index_signature)
            }
            _ => None,
        })
        .next_back();

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
        string_index_type,
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
    let property = match member {
        TSSignature::TSPropertySignature(property_signature) => {
            parse_type_property_signature(property_signature)?
        }
        TSSignature::TSMethodSignature(method_signature) => {
            parse_type_method_signature(method_signature)?
        }
        _ => return None,
    };

    Some(ParsedInterfaceMember {
        name: property.name,
        name_span: property.name_span,
        optional: property.optional,
        ty: property.ty,
    })
}
