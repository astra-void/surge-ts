use oxc_ast::ast::{
    Class, ClassElement, MethodDefinitionKind, MethodDefinitionType,
    PropertyDefinitionType, PropertyKey,
};

use oxc_span::GetSpan;

use crate::{
    ParsedClassAccessor, ParsedClassConstructor, ParsedClassDeclaration, ParsedClassMember,
    ParsedAccessorSide, ParsedClassMethod, ParsedClassProperty, ParsedMemberAccessibility,
    ParsedNamedType, ParsedRestrictedMember,
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

    // The last instance signature of each key kind wins, as for an interface.
    // A `symbol` or pattern key answers no named member (Prisma's client class
    // declares `[K: symbol]`), so only `string` and `number` keys are kept.
    let index_signature_of = |numeric: bool| {
        class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::TSIndexSignature(index_signature)
                    if !index_signature.r#static
                        && index_signature.parameters.first().is_some_and(|parameter| {
                            matches!(
                                parameter.type_annotation.type_annotation,
                                oxc_ast::ast::TSType::TSStringKeyword(_)
                                    | oxc_ast::ast::TSType::TSNumberKeyword(_)
                            )
                        })
                        && super::types::index_signature_is_numeric(index_signature) == numeric =>
                {
                    super::types::parse_index_signature_value_type(index_signature)
                }
                _ => None,
            })
            .next_back()
    };

    Some(ParsedClassDeclaration {
        string_index_type: index_signature_of(false),
        number_index_type: index_signature_of(true),
        is_declare: class.declare,
        is_abstract: class.r#abstract,
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
        type_parameters: parse_type_parameters(class.type_parameters.as_deref()),
        extends: parse_class_heritage(class),
        implements: class
            .implements
            .iter()
            .filter_map(|implemented| {
                let (name, span) = super::types::flatten_type_name(&implemented.expression)?;
                Some(ParsedNamedType {
                    name,
                    span: Some(span),
                    type_arguments: implemented
                        .type_arguments
                        .as_deref()
                        .and_then(super::types::parse_type_arguments)
                        .unwrap_or_default(),
                })
            })
            .collect(),
        members,
        restricted_members: restricted_class_members(class),
        span: Some(text_span_from_oxc_span(class.span)),
    })
}

fn restricted_class_members(class: &Class<'_>) -> Vec<ParsedRestrictedMember> {
    let mut restricted = Vec::new();
    for element in &class.body.body {
        let (key, computed, is_static, accessibility, accessor_side) = match element {
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor =>
            {
                for parameter in &method.value.params.items {
                    if let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) =
                        &parameter.pattern
                        && let Some(accessibility) = restricted_accessibility(parameter.accessibility)
                    {
                        restricted.push(ParsedRestrictedMember {
                            name: identifier.name.to_string(),
                            is_static: false,
                            accessibility,
                            accessor_side: None,
                        });
                    }
                }
                continue;
            }
            ClassElement::MethodDefinition(method) => (
                &method.key,
                method.computed,
                method.r#static,
                method.accessibility,
                match method.kind {
                    MethodDefinitionKind::Get => Some(ParsedAccessorSide::Get),
                    MethodDefinitionKind::Set => Some(ParsedAccessorSide::Set),
                    _ => None,
                },
            ),
            ClassElement::PropertyDefinition(property) => (
                &property.key,
                property.computed,
                property.r#static,
                property.accessibility,
                None,
            ),
            ClassElement::AccessorProperty(property) => (
                &property.key,
                property.computed,
                property.r#static,
                property.accessibility,
                None,
            ),
            _ => continue,
        };
        let Some(accessibility) = restricted_accessibility(accessibility) else {
            continue;
        };
        let name = if computed {
            super::types::computed_key_name(key)
        } else {
            match key {
                PropertyKey::StaticIdentifier(key) => Some(key.name.to_string()),
                key => super::types::computed_key_name(key),
            }
        };
        if let Some(name) = name {
            restricted.push(ParsedRestrictedMember {
                name,
                is_static,
                accessibility,
                accessor_side,
            });
        }
    }
    restricted
}

fn restricted_accessibility(
    accessibility: Option<oxc_ast::ast::TSAccessibility>,
) -> Option<ParsedMemberAccessibility> {
    match accessibility? {
        oxc_ast::ast::TSAccessibility::Private => Some(ParsedMemberAccessibility::Private),
        oxc_ast::ast::TSAccessibility::Protected => Some(ParsedMemberAccessibility::Protected),
        oxc_ast::ast::TSAccessibility::Public => None,
    }
}

fn parse_class_heritage(class: &Class<'_>) -> Vec<ParsedNamedType> {
    let Some(super_class) = class.super_class.as_ref() else {
        return Vec::new();
    };
    let Some((name, span)) = super::types::flatten_heritage_expression(super_class) else {
        return Vec::new();
    };

    let type_arguments = class
        .super_type_arguments
        .as_deref()
        .and_then(parse_type_arguments)
        .unwrap_or_default();

    vec![ParsedNamedType {
        name,
        span: Some(span),
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
                existing.declarations.extend(accessor.declarations);
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
            // A computed constructor is not a constructor; a computed method or
            // accessor keeps the `[…]` name the type side uses
            // (`[Symbol.iterator]()`, `get [matcher]()`).
            if method.computed && method.kind == MethodDefinitionKind::Constructor {
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
            let body_reads = method
                .value
                .body
                .as_ref()
                .map(|body| super::reads::collect_function_body_reads(body))
                .unwrap_or_default();

            match method.kind {
                MethodDefinitionKind::Constructor => {
                    Some(ParsedClassMember::Constructor(ParsedClassConstructor {
                        parameters,
                        body,
                        body_reads,
                        span: Some(text_span_from_oxc_span(method.span)),
                    }))
                }
                MethodDefinitionKind::Method => {
                    let (name, name_span) = if method.computed {
                        (super::types::computed_key_name(&method.key)?, method.key.span())
                    } else {
                        match &method.key {
                            PropertyKey::StaticIdentifier(key) => (key.name.to_string(), key.span),
                            // `1: T` and `"a": T` name members as their computed forms do.
                            key => (super::types::computed_key_name(key)?, key.span()),
                        }
                    };
                    let return_type = method
                        .value
                        .return_type
                        .as_ref()
                        .and_then(|annotation| parse_type_annotation(annotation));
                    let return_type_span = method.value.return_type.as_ref().map(|annotation| {
                        text_span_from_oxc_span(annotation.type_annotation.span())
                    });

                    Some(ParsedClassMember::Method(ParsedClassMethod {
                        name,
                        name_span: Some(text_span_from_oxc_span(name_span)),
                        is_static: method.r#static,
                        is_override: method.r#override,
                        is_abstract: matches!(
                            method.r#type,
                            MethodDefinitionType::TSAbstractMethodDefinition
                        ),
                        type_parameters: parse_type_parameters(
                            method.value.type_parameters.as_deref(),
                        ),
                        parameters,
                        return_type,
                        return_type_span,
                        body,
                        has_body: method.value.body.is_some(),
                        is_generator: method.value.generator,
                        is_async: method.value.r#async,
                        body_reads,
                    }))
                }
                MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                    let (name, name_span) = if method.computed {
                        (super::types::computed_key_name(&method.key)?, method.key.span())
                    } else {
                        match &method.key {
                            PropertyKey::StaticIdentifier(key) => (key.name.to_string(), key.span),
                            // `1: T` and `"a": T` name members as their computed forms do.
                            key => (super::types::computed_key_name(key)?, key.span()),
                        }
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
                        name,
                        name_span: Some(text_span_from_oxc_span(name_span)),
                        is_static: method.r#static,
                        is_override: method.r#override,
                        is_abstract: matches!(
                            method.r#type,
                            MethodDefinitionType::TSAbstractMethodDefinition
                        ),
                        getter_return_type,
                        setter_param_type,
                        has_getter: is_getter,
                        has_setter: !is_getter,
                        declarations: vec![crate::ParsedAccessorDeclaration {
                            is_getter,
                            parameters,
                            return_type_span: method.value.return_type.as_ref().map(
                                |annotation| text_span_from_oxc_span(annotation.type_annotation.span()),
                            ),
                            body,
                            body_reads,
                            has_body: method.value.body.is_some(),
                        }],
                    }))
                }
            }
        }
        ClassElement::PropertyDefinition(property) => {
            let (name, name_span) = if property.computed {
                (super::types::computed_key_name(&property.key)?, property.key.span())
            } else {
                match &property.key {
                    PropertyKey::StaticIdentifier(key) => (key.name.to_string(), key.span),
                    // `1: T` and `"a": T` name members as their computed forms do.
                    key => (super::types::computed_key_name(key)?, key.span()),
                }
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
                name,
                name_span: Some(text_span_from_oxc_span(name_span)),
                is_static: property.r#static,
                is_override: property.r#override,
                is_abstract: matches!(
                    property.r#type,
                    PropertyDefinitionType::TSAbstractPropertyDefinition
                ),
                is_declare: property.declare,
                has_definite_assertion: property.definite,
                optional: property.optional,
                readonly: property.readonly,
                declared_type,
                initializer,
                initializer_span,
            }))
        }
        ClassElement::StaticBlock(block) => {
            Some(ParsedClassMember::StaticBlock(crate::ParsedClassStaticBlock {
                body: parse_statement_list_as_function_body(&block.body),
                body_reads: super::reads::collect_statement_reads(&block.body),
            }))
        }
        // Index signatures and accessor properties are not part of this slice.
        ClassElement::AccessorProperty(_) | ClassElement::TSIndexSignature(_) => None,
    }
}
