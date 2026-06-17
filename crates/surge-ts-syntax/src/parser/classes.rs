use oxc_ast::ast::{Class, ClassElement, Expression, MethodDefinitionKind, PropertyKey};

use crate::{
    ParsedClassAccessor, ParsedClassConstructor, ParsedClassDeclaration, ParsedClassMember,
    ParsedClassMethod, ParsedClassProperty, ParsedNamedType,
};

use super::expressions::parse_expression;
use super::functions::{
    parse_function_parameter, parse_rest_function_parameter, parse_statement_list_as_function_body,
};
use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_arguments, parse_type_parameters};

pub(crate) fn parse_class_declaration(class: &Class<'_>) -> Option<ParsedClassDeclaration> {
    let id = class.id.as_ref()?;

    let members = merge_class_accessors(
        class
            .body
            .body
            .iter()
            .filter_map(parse_class_member)
            .collect(),
    );

    Some(ParsedClassDeclaration {
        is_declare: class.declare,
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
        type_parameters: parse_type_parameters(class.type_parameters.as_deref()),
        extends: parse_class_heritage(class),
        members,
        span: Some(text_span_from_oxc_span(class.span)),
    })
}

fn parse_class_heritage(class: &Class<'_>) -> Vec<ParsedNamedType> {
    let Some(Expression::Identifier(identifier)) = class.super_class.as_ref() else {
        return Vec::new();
    };

    let type_arguments = class
        .super_type_arguments
        .as_deref()
        .and_then(parse_type_arguments)
        .unwrap_or_default();

    vec![ParsedNamedType {
        name: identifier.name.to_string(),
        span: Some(text_span_from_oxc_span(identifier.span)),
        type_arguments,
    }]
}

/// Collapses separate `get`/`set` accessor members that share a name into a
/// single accessor member (a getter and its matching setter become one).
fn merge_class_accessors(members: Vec<ParsedClassMember>) -> Vec<ParsedClassMember> {
    let mut merged: Vec<ParsedClassMember> = Vec::with_capacity(members.len());

    for member in members {
        let ParsedClassMember::Accessor(accessor) = member else {
            merged.push(member);
            continue;
        };

        let existing = merged.iter_mut().find_map(|candidate| match candidate {
            ParsedClassMember::Accessor(existing)
                if existing.name == accessor.name && existing.is_static == accessor.is_static =>
            {
                Some(existing)
            }
            _ => None,
        });

        match existing {
            Some(existing) => {
                if accessor.has_getter {
                    existing.has_getter = true;
                    existing.getter_return_type = accessor.getter_return_type;
                }
                if accessor.has_setter {
                    existing.has_setter = true;
                    existing.setter_param_type = accessor.setter_param_type;
                }
            }
            None => merged.push(ParsedClassMember::Accessor(accessor)),
        }
    }

    merged
}

fn parse_class_member(member: &ClassElement<'_>) -> Option<ParsedClassMember> {
    match member {
        ClassElement::MethodDefinition(method) => {
            if method.computed {
                return None;
            }

            let mut parameters: Vec<_> = method
                .value
                .params
                .items
                .iter()
                .filter_map(parse_function_parameter)
                .collect();
            if let Some(rest) = method.value.params.rest.as_deref() {
                if let Some(rest_parameter) = parse_rest_function_parameter(rest) {
                    parameters.push(rest_parameter);
                }
            }
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
                MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                    let PropertyKey::StaticIdentifier(key) = &method.key else {
                        return None;
                    };

                    let is_getter = matches!(method.kind, MethodDefinitionKind::Get);
                    let getter_return_type = if is_getter {
                        method
                            .value
                            .return_type
                            .as_ref()
                            .and_then(|annotation| parse_type_annotation(annotation))
                    } else {
                        None
                    };
                    let setter_param_type = if is_getter {
                        None
                    } else {
                        method
                            .value
                            .params
                            .items
                            .first()
                            .and_then(|param| param.type_annotation.as_ref())
                            .and_then(|annotation| parse_type_annotation(annotation))
                    };

                    Some(ParsedClassMember::Accessor(ParsedClassAccessor {
                        name: key.name.to_string(),
                        name_span: Some(text_span_from_oxc_span(key.span)),
                        is_static: method.r#static,
                        getter_return_type,
                        setter_param_type,
                        has_getter: is_getter,
                        has_setter: !is_getter,
                    }))
                }
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
