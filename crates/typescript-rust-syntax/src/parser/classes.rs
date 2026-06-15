use oxc_ast::ast::{Class, ClassElement, MethodDefinitionKind, PropertyKey};

use crate::{
    ParsedClassConstructor, ParsedClassDeclaration, ParsedClassMember, ParsedClassMethod,
    ParsedClassProperty,
};

use super::expressions::parse_expression;
use super::functions::{parse_function_parameter, parse_statement_list_as_function_body};
use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_parameters};

pub(crate) fn parse_class_declaration(class: &Class<'_>) -> Option<ParsedClassDeclaration> {
    let id = class.id.as_ref()?;

    let members = class
        .body
        .body
        .iter()
        .filter_map(parse_class_member)
        .collect();

    Some(ParsedClassDeclaration {
        is_declare: class.declare,
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
        type_parameters: parse_type_parameters(class.type_parameters.as_deref()),
        members,
        span: Some(text_span_from_oxc_span(class.span)),
    })
}

fn parse_class_member(member: &ClassElement<'_>) -> Option<ParsedClassMember> {
    match member {
        ClassElement::MethodDefinition(method) => {
            if method.computed {
                return None;
            }

            let parameters = method
                .value
                .params
                .items
                .iter()
                .filter_map(parse_function_parameter)
                .collect();
            let body = method
                .value
                .body
                .as_ref()
                .map(|body| parse_statement_list_as_function_body(&body.statements))
                .unwrap_or_default();

            match method.kind {
                MethodDefinitionKind::Constructor => {
                    Some(ParsedClassMember::Constructor(ParsedClassConstructor {
                        parameters,
                        body,
                        span: Some(text_span_from_oxc_span(method.span)),
                    }))
                }
                MethodDefinitionKind::Method => {
                    let PropertyKey::StaticIdentifier(key) = &method.key else {
                        return None;
                    };
                    let return_type = method
                        .value
                        .return_type
                        .as_ref()
                        .and_then(|annotation| parse_type_annotation(annotation));

                    Some(ParsedClassMember::Method(ParsedClassMethod {
                        name: key.name.to_string(),
                        name_span: Some(text_span_from_oxc_span(key.span)),
                        is_static: method.r#static,
                        type_parameters: parse_type_parameters(
                            method.value.type_parameters.as_deref(),
                        ),
                        parameters,
                        return_type,
                        body,
                    }))
                }
                // Getters and setters are not modelled in this slice; drop them
                // rather than failing to parse the class.
                MethodDefinitionKind::Get | MethodDefinitionKind::Set => None,
            }
        }
        ClassElement::PropertyDefinition(property) => {
            if property.computed {
                return None;
            }

            let PropertyKey::StaticIdentifier(key) = &property.key else {
                return None;
            };

            let declared_type = property
                .type_annotation
                .as_ref()
                .and_then(|annotation| parse_type_annotation(annotation));
            let (initializer, initializer_span) = match property.value.as_ref() {
                Some(value) => {
                    let (expression, span) = parse_expression(value);
                    (Some(expression), Some(text_span_from_oxc_span(span)))
                }
                None => (None, None),
            };

            Some(ParsedClassMember::Property(ParsedClassProperty {
                name: key.name.to_string(),
                name_span: Some(text_span_from_oxc_span(key.span)),
                is_static: property.r#static,
                optional: property.optional,
                readonly: property.readonly,
                declared_type,
                initializer,
                initializer_span,
            }))
        }
        // Static blocks, index signatures, and accessor properties are not part of
        // this slice.
        ClassElement::StaticBlock(_)
        | ClassElement::AccessorProperty(_)
        | ClassElement::TSIndexSignature(_) => None,
    }
}
