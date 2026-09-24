use oxc_ast::ast::{
    Class, ClassElement, Expression, MethodDefinitionKind, MethodDefinitionType,
    PropertyDefinitionType, PropertyKey,
};
use oxc_syntax::operator::BinaryOperator;

use oxc_span::GetSpan;

use crate::{
    ParsedClassAccessor, ParsedClassConstructor, ParsedClassDeclaration, ParsedClassMember,
    ParsedAccessorSide, ParsedClassMethod, ParsedClassProperty, ParsedDecorator,
    ParsedDecoratorTarget, ParsedMemberAccessibility, ParsedNamedType, ParsedRestrictedMember,
};

use super::expressions::parse_expression;
use super::functions::{
    parse_function_parameter, parse_rest_function_parameter, parse_statement_list_as_function_body,
};
use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_arguments, parse_type_parameters};

pub(crate) fn parse_class_declaration(class: &Class<'_>) -> Option<ParsedClassDeclaration> {
    let id = class.id.as_ref()?;

    let mut members = merge_class_accessors(
        class
            .body
            .body
            .iter()
            .filter_map(parse_class_member)
            .collect(),
    );
    if super::spans::lowering_javascript() {
        let assigned = javascript_this_members(class, &members);
        members.extend(assigned);
    }

    // The last signature of each key kind and side wins, as for an interface.
    // A `symbol` or pattern key answers no named member (Prisma's client class
    // declares `[K: symbol]`), so only `string` and `number` keys are kept.
    let index_signature_of = |numeric: bool, is_static: bool| {
        class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::TSIndexSignature(index_signature)
                    if index_signature.r#static == is_static
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
                        .map(|value_type| (value_type, text_span_from_oxc_span(index_signature.span)))
                }
                _ => None,
            })
            .next_back()
    };
    let (string_index_type, string_index_span) = index_signature_of(false, false).unzip();
    let (number_index_type, number_index_span) = index_signature_of(true, false).unzip();
    let static_string_index_type = index_signature_of(false, true).map(|(ty, _)| ty);
    let static_number_index_type = index_signature_of(true, true).map(|(ty, _)| ty);

    Some(ParsedClassDeclaration {
        string_index_type,
        number_index_type,
        string_index_span,
        number_index_span,
        static_string_index_type,
        static_number_index_type,
        is_declare: class.declare,
        is_abstract: class.r#abstract,
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
        type_parameters: parse_type_parameters(class.type_parameters.as_deref()),
        extends: parse_class_heritage(class),
        heritage_expression: class
            .super_class
            .as_ref()
            .filter(|super_class| super::types::flatten_heritage_expression(super_class).is_none())
            .map(|super_class| Box::new(parse_expression(super_class).0)),
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
        computed_keys: class
            .body
            .body
            .iter()
            .filter_map(|element| {
                let (key, computed, is_get_or_set) = match element {
                    ClassElement::MethodDefinition(member) => (
                        &member.key,
                        member.computed,
                        member.kind.is_accessor(),
                    ),
                    ClassElement::PropertyDefinition(member) => (&member.key, member.computed, false),
                    ClassElement::AccessorProperty(member) => (&member.key, member.computed, false),
                    _ => return None,
                };
                let expression = key.as_expression().filter(|_| computed)?;
                // tsc's `isInvalidComputedPropertyName`: a `[k in T]` name is a
                // misplaced mapped type (TS7061) and its expression is never checked.
                if !is_get_or_set && is_in_expression(expression) {
                    return None;
                }
                let (parsed, _) = parse_expression(expression);
                // The name's `[`, which the key's own span leaves out.
                let span = key.span();
                Some((
                    parsed,
                    Some(crate::TextSpan {
                        start: (span.start as usize).saturating_sub(1),
                        end: span.end as usize + 1,
                    }),
                ))
            })
            .collect(),
        decorators: class_decorators(class),
    })
}

fn class_decorators(class: &Class<'_>) -> Vec<ParsedDecorator> {
    let lower = |decorators: &[oxc_ast::ast::Decorator<'_>], target: ParsedDecoratorTarget| {
        decorators
            .iter()
            .map(|decorator| ParsedDecorator {
                expression: parse_expression(&decorator.expression).0,
                span: Some(text_span_from_oxc_span(decorator.expression.span())),
                target,
            })
            .collect::<Vec<_>>()
    };
    let mut out = lower(&class.decorators, ParsedDecoratorTarget::Class);
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                let target = ParsedDecoratorTarget::Method {
                    has_body: method.value.body.is_some(),
                    private_name: matches!(method.key, PropertyKey::PrivateIdentifier(_)),
                };
                out.extend(lower(&method.decorators, target));
                if method.value.body.is_some() && method.kind != MethodDefinitionKind::Get {
                    for parameter in &method.value.params.items {
                        out.extend(lower(&parameter.decorators, ParsedDecoratorTarget::Parameter));
                    }
                }
            }
            ClassElement::PropertyDefinition(property) => {
                let target = ParsedDecoratorTarget::Property {
                    is_abstract: property.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition,
                    is_declare: property.declare,
                    private_name: matches!(property.key, PropertyKey::PrivateIdentifier(_)),
                };
                out.extend(lower(&property.decorators, target));
            }
            ClassElement::AccessorProperty(accessor) => {
                let target = ParsedDecoratorTarget::Property {
                    is_abstract: accessor.r#type == oxc_ast::ast::AccessorPropertyType::TSAbstractAccessorProperty,
                    is_declare: false,
                    private_name: matches!(accessor.key, PropertyKey::PrivateIdentifier(_)),
                };
                out.extend(lower(&accessor.decorators, target));
            }
            _ => {}
        }
    }
    out
}

fn is_in_expression(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::BinaryExpression(binary) => binary.operator == BinaryOperator::In,
        Expression::PrivateInExpression(_) => true,
        _ => false,
    }
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
        return vec![ParsedNamedType {
            name: crate::EXPRESSION_HERITAGE_BASE.to_string(),
            span: Some(text_span_from_oxc_span(super_class.span())),
            type_arguments: Vec::new(),
        }];
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
                        accessibility: restricted_accessibility(method.accessibility),
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
                        span: Some(text_span_from_oxc_span(method.span)),
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
                        this_parameter_type: method
                            .value
                            .this_param
                            .as_ref()
                            .and_then(|this_param| this_param.type_annotation.as_ref())
                            .and_then(|annotation| parse_type_annotation(annotation)),
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
                        span: Some(text_span_from_oxc_span(method.span)),
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
            let has_literal_name = !property.computed && is_literal_property_key(&property.key);
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
                span: Some(text_span_from_oxc_span(property.span)),
                name,
                name_span: Some(text_span_from_oxc_span(name_span)),
                has_literal_name,
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
                this_assignments: None,
            }))
        }
        ClassElement::StaticBlock(block) => {
            Some(ParsedClassMember::StaticBlock(crate::ParsedClassStaticBlock {
                span: Some(text_span_from_oxc_span(block.span)),
                body: parse_statement_list_as_function_body(&block.body),
                body_reads: super::reads::collect_statement_reads(&block.body),
            }))
        }
        // An auto-accessor (`accessor x: T = v`) is a get/set pair over a private
        // slot: for typing it is the property it looks like. Dropping it made
        // every use of the member a missing property.
        ClassElement::AccessorProperty(property) => {
            let has_literal_name = !property.computed && is_literal_property_key(&property.key);
            let (name, name_span) = if property.computed {
                (super::types::computed_key_name(&property.key)?, property.key.span())
            } else {
                match &property.key {
                    PropertyKey::StaticIdentifier(key) => (key.name.to_string(), key.span),
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
                span: Some(text_span_from_oxc_span(property.span)),
                name,
                name_span: Some(text_span_from_oxc_span(name_span)),
                has_literal_name,
                is_static: property.r#static,
                is_override: property.r#override,
                is_abstract: matches!(
                    property.r#type,
                    oxc_ast::ast::AccessorPropertyType::TSAbstractAccessorProperty
                ),
                is_declare: false,
                has_definite_assertion: property.definite,
                optional: false,
                readonly: false,
                declared_type,
                initializer,
                initializer_span,
                this_assignments: None,
            }))
        }
        // Index signatures are not part of this slice.
        ClassElement::TSIndexSignature(_) => None,
    }
}

/// tsc's `bindThisPropertyAssignment` for a JavaScript class: `this.x = v` in
/// a constructor, method, accessor, static block or property initializer — or
/// an arrow function within one — declares `x` on the instance (on the class,
/// in a static one) unless the class declares it. An assignment whose value
/// reads the same member declares it without typing it
/// (`containsSameNamedThisProperty`).
fn javascript_this_members(
    class: &Class<'_>,
    declared: &[ParsedClassMember],
) -> Vec<ParsedClassMember> {
    use oxc_ast::ast::{AssignmentTarget, Expression};
    use oxc_ast_visit::{Visit, walk};

    struct Found {
        name: String,
        name_span: oxc_span::Span,
        value: Option<crate::ParsedExpression>,
        is_static: bool,
        in_constructor: bool,
    }
    struct Collector {
        found: Vec<Found>,
        is_static: bool,
        in_constructor: bool,
    }
    impl<'a> Visit<'a> for Collector {
        fn visit_assignment_expression(&mut self, assignment: &oxc_ast::ast::AssignmentExpression<'a>) {
            if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                && let AssignmentTarget::StaticMemberExpression(member) = &assignment.left
                && matches!(member.object, Expression::ThisExpression(_))
            {
                self.found.push(Found {
                    name: member.property.name.to_string(),
                    name_span: member.property.span,
                    value: (!reads_same_this_member(&assignment.right, &member.property.name))
                        .then(|| parse_expression(&assignment.right).0),
                    is_static: self.is_static,
                    in_constructor: self.in_constructor,
                });
            }
            walk::walk_assignment_expression(self, assignment);
        }
        // A nested function or class has a `this` of its own.
        fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_class(&mut self, _: &Class<'a>) {}
    }

    let mut collector = Collector { found: Vec::new(), is_static: false, in_constructor: false };
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                let Some(body) = &method.value.body else { continue };
                collector.is_static = method.r#static;
                collector.in_constructor = method.kind == MethodDefinitionKind::Constructor;
                collector.visit_function_body(body);
            }
            ClassElement::StaticBlock(block) => {
                collector.is_static = true;
                collector.in_constructor = true;
                for statement in &block.body {
                    collector.visit_statement(statement);
                }
            }
            ClassElement::PropertyDefinition(property) => {
                let Some(value) = &property.value else { continue };
                collector.is_static = property.r#static;
                collector.in_constructor = false;
                collector.visit_expression(value);
            }
            _ => {}
        }
    }

    let is_declared = |name: &str, is_static: bool| {
        declared.iter().any(|member| match member {
            ParsedClassMember::Property(property) => property.name == name && property.is_static == is_static,
            ParsedClassMember::Method(method) => method.name == name && method.is_static == is_static,
            ParsedClassMember::Accessor(accessor) => accessor.name == name && accessor.is_static == is_static,
            _ => false,
        })
    };
    let mut members: Vec<ParsedClassMember> = Vec::new();
    let mut seen: Vec<(String, bool)> = Vec::new();
    for found in &collector.found {
        let key = (found.name.clone(), found.is_static);
        if seen.contains(&key) || is_declared(&found.name, found.is_static) {
            continue;
        }
        seen.push(key);
        let assignments: Vec<&Found> = collector
            .found
            .iter()
            .filter(|other| other.name == found.name && other.is_static == found.is_static)
            .collect();
        let in_constructor = assignments.iter().any(|assignment| assignment.in_constructor);
        let values = assignments
            .iter()
            .filter(|assignment| !in_constructor || assignment.in_constructor)
            .filter_map(|assignment| assignment.value.clone())
            .collect();
        let span = Some(text_span_from_oxc_span(found.name_span));
        members.push(ParsedClassMember::Property(ParsedClassProperty {
            span,
            name: found.name.clone(),
            name_span: span,
            has_literal_name: false,
            is_static: found.is_static,
            is_override: false,
            is_abstract: false,
            is_declare: false,
            has_definite_assertion: false,
            optional: false,
            readonly: false,
            declared_type: None,
            initializer: None,
            initializer_span: None,
            this_assignments: Some(crate::ParsedThisAssignments { values, in_constructor }),
        }));
    }
    members
}

/// tsc's `containsSameNamedThisProperty`: whether `this.<name>` is one of the
/// value's direct operands (`this.x = this.x || {}`).
fn reads_same_this_member(value: &oxc_ast::ast::Expression<'_>, name: &str) -> bool {
    use oxc_ast::ast::Expression;
    let is_same = |operand: &Expression<'_>| {
        matches!(operand, Expression::StaticMemberExpression(member)
            if matches!(member.object, Expression::ThisExpression(_)) && member.property.name == name)
    };
    match value {
        Expression::LogicalExpression(logical) => is_same(&logical.left) || is_same(&logical.right),
        Expression::BinaryExpression(binary) => is_same(&binary.left) || is_same(&binary.right),
        Expression::ConditionalExpression(conditional) => {
            is_same(&conditional.test) || is_same(&conditional.consequent) || is_same(&conditional.alternate)
        }
        Expression::CallExpression(call) => {
            is_same(&call.callee)
                || call.arguments.iter().any(|argument| argument.as_expression().is_some_and(is_same))
        }
        other => is_same(other),
    }
}

fn is_literal_property_key(key: &PropertyKey<'_>) -> bool {
    matches!(key, PropertyKey::StringLiteral(_) | PropertyKey::NumericLiteral(_))
}
