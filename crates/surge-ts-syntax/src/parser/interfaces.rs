use oxc_ast::ast::{TSInterfaceDeclaration, TSInterfaceHeritage, TSSignature};

use crate::{ParsedInterfaceDeclaration, ParsedInterfaceMember};

use super::spans::text_span_from_oxc_span;
use super::types::parse_type_parameters;
use super::types::{
    parse_call_signature, parse_construct_signature, parse_index_signature_value_type,
    parse_type_method_signature, parse_type_property_signature,
};

pub(crate) fn parse_interface_declaration(
    declaration: &TSInterfaceDeclaration<'_>,
) -> Option<ParsedInterfaceDeclaration> {
    let getters = super::types::getter_accessor_names(&declaration.body.body);
    let members = declaration
        .body
        .body
        .iter()
        .filter(|member| !super::types::is_shadowed_setter(member, &getters))
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

    // A bare call signature (`(value?: any): number`) makes the interface
    // callable. When an interface declares multiple call-signature overloads we
    // keep the first that parses; the call checker only needs one viable arity.
    let call_signature = declaration
        .body
        .body
        .iter()
        .find_map(|member| match member {
            TSSignature::TSCallSignatureDeclaration(signature) => parse_call_signature(signature),
            _ => None,
        });

    // Construct signatures (`new <T>(...): Promise<T>`) make the interface usable
    // with `new` (e.g. `PromiseConstructor`). Collect every overload; the resolver
    // merges them into one permissive signature.
    let construct_signatures = declaration
        .body
        .body
        .iter()
        .filter_map(|member| match member {
            TSSignature::TSConstructSignatureDeclaration(signature) => {
                parse_construct_signature(signature)
            }
            _ => None,
        })
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
        string_index_type,
        call_signature,
        construct_signatures,
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
        is_abstract: false,
        ty: property.ty,
    })
}
