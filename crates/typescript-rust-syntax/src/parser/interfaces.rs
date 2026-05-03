use oxc_ast::ast::{TSInterfaceDeclaration, TSSignature};

use crate::{ParsedInterfaceDeclaration, ParsedInterfaceMember};

use super::spans::text_span_from_oxc_span;
use super::types::parse_type_parameters;
use super::types::parse_type_property_signature;

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
        members,
    })
}

fn parse_interface_member(member: &TSSignature<'_>) -> Option<ParsedInterfaceMember> {
    let TSSignature::TSPropertySignature(property_signature) = member else {
        return None;
    };

    let property = parse_type_property_signature(property_signature)?;

    Some(ParsedInterfaceMember {
        name: property.name,
        name_span: property.name_span,
        optional: property.optional,
        ty: property.ty,
    })
}
