use oxc_ast::ast::{
    Argument, ArrayExpression, ArrowFunctionExpression, BinaryExpression, BinaryOperator,
    ChainElement, ChainExpression, ComputedMemberExpression, ConditionalExpression, Expression,
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
    JSXExpressionContainer, JSXFragment, JSXMemberExpression, JSXMemberExpressionObject,
    LogicalExpression, LogicalOperator, NewExpression, ObjectExpression, ObjectPropertyKind,
    PropertyKey, PropertyKind, StaticMemberExpression, UnaryExpression, UnaryOperator,
};
use oxc_span::{GetSpan, Span};

use crate::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedBinaryOperator, ParsedCall,
    ParsedCallArgument, ParsedExpression, ParsedJsxAttribute, ParsedJsxChild,
    ParsedLogicalOperator, ParsedObjectProperty, ParsedUnaryOperator, TextSpan,
};

use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_arguments, parse_type_parameters};
use super::{
    functions::parse_function_parameter, functions::parse_statement_list_as_function_body,
};

pub(crate) fn parse_expression(expression: &Expression<'_>) -> (ParsedExpression, Span) {
    let parsed_expression = match expression {
        Expression::StringLiteral(string_literal) => {
            ParsedExpression::StringLiteral(string_literal.value.to_string())
        }
        Expression::NumericLiteral(numeric_literal) => {
            ParsedExpression::NumberLiteral(numeric_literal.value.to_string())
        }
        Expression::BooleanLiteral(boolean_literal) => {
            ParsedExpression::BooleanLiteral(boolean_literal.value)
        }
        Expression::NullLiteral(_) => ParsedExpression::NullLiteral,
        Expression::Identifier(identifier) => {
            if identifier.name == "undefined" {
                ParsedExpression::UndefinedLiteral
            } else {
                ParsedExpression::Identifier {
                    name: identifier.name.to_string(),
                    span: Some(text_span_from_oxc_span(identifier.span)),
                }
            }
        }
        Expression::ObjectExpression(object_expression) => ParsedExpression::ObjectLiteral {
            properties: parse_object_properties(object_expression),
            span: Some(text_span_from_oxc_span(object_expression.span())),
        },
        Expression::ArrayExpression(array_expression) => {
            parse_array_expression(array_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::BinaryExpression(binary_expression) => {
            parse_binary_expression(binary_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::LogicalExpression(logical_expression) => {
            parse_logical_expression(logical_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::UnaryExpression(unary_expression) => {
            parse_unary_expression(unary_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::ParenthesizedExpression(parenthesized_expression) => {
            return parse_expression(&parenthesized_expression.expression);
        }
        Expression::AwaitExpression(await_expression) => {
            return parse_expression(&await_expression.argument);
        }
        Expression::ConditionalExpression(conditional_expression) => {
            parse_conditional_expression(conditional_expression)
                .unwrap_or(ParsedExpression::Unknown)
        }
        Expression::CallExpression(call_expression) => {
            parse_call_expression_expression(call_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::NewExpression(new_expression) => {
            parse_new_expression(new_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::TSInstantiationExpression(instantiation_expression) => {
            parse_instantiation_expression(instantiation_expression)
                .unwrap_or(ParsedExpression::Unknown)
        }
        Expression::StaticMemberExpression(member_expression) => {
            parse_static_member_expression(member_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::ComputedMemberExpression(member_expression) => {
            parse_computed_member_expression(member_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::TSAsExpression(as_expression) => {
            let (expression, expression_span) = parse_expression(&as_expression.expression);
            let ty = crate::parser::types::parse_type(&as_expression.type_annotation)
                .unwrap_or(crate::ParsedType::Unknown);

            if let crate::ParsedType::Named(named_type) = &ty {
                if named_type.name == "const" && named_type.type_arguments.is_empty() {
                    return (
                        ParsedExpression::ConstAssertion {
                            expression: Box::new(expression),
                            span: Some(text_span_from_oxc_span(as_expression.span)),
                        },
                        as_expression.span,
                    );
                }
            }

            ParsedExpression::TypeAssertion {
                expression: Box::new(expression),
                expression_span: Some(text_span_from_oxc_span(expression_span)),
                ty,
                type_span: Some(text_span_from_oxc_span(
                    as_expression.type_annotation.span(),
                )),
            }
        }
        Expression::TSSatisfiesExpression(satisfies_expression) => {
            let (expression, expression_span) = parse_expression(&satisfies_expression.expression);
            let ty = crate::parser::types::parse_type(&satisfies_expression.type_annotation)
                .unwrap_or(crate::ParsedType::Unknown);
            ParsedExpression::SatisfiesExpression {
                expression: Box::new(expression),
                span: Some(text_span_from_oxc_span(expression_span)),
                target_type: ty,
                target_span: Some(text_span_from_oxc_span(
                    satisfies_expression.type_annotation.span(),
                )),
            }
        }
        Expression::TSNonNullExpression(non_null_expression) => {
            let (expression, _expression_span) = parse_expression(&non_null_expression.expression);
            ParsedExpression::NonNullAssertion {
                expression: Box::new(expression),
                span: Some(text_span_from_oxc_span(non_null_expression.span)),
                in_optional_chain: false,
            }
        }
        Expression::ChainExpression(chain_expression) => {
            parse_chain_expression(chain_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::ArrowFunctionExpression(arrow_expression) => {
            parse_arrow_function_expression(arrow_expression)
                .map(|arrow| ParsedExpression::ArrowFunction(Box::new(arrow)))
                .unwrap_or(ParsedExpression::Unknown)
        }
        Expression::JSXElement(jsx_element) => parse_jsx_element(jsx_element),
        Expression::JSXFragment(jsx_fragment) => parse_jsx_fragment(jsx_fragment),
        _ => ParsedExpression::Unknown,
    };

    (parsed_expression, expression.span())
}

fn parse_jsx_element(element: &JSXElement<'_>) -> ParsedExpression {
    let opening = &element.opening_element;
    let (tag_name, tag_name_span, component_name, component_span) =
        parse_jsx_element_name(&opening.name);
    let attributes = opening
        .attributes
        .iter()
        .filter_map(parse_jsx_attribute_item)
        .collect();
    let children = element.children.iter().map(parse_jsx_child).collect();

    ParsedExpression::JsxElement {
        tag_name,
        tag_name_span,
        component_name,
        component_span,
        attributes,
        children,
        span: Some(text_span_from_oxc_span(element.span)),
    }
}

fn parse_jsx_fragment(fragment: &JSXFragment<'_>) -> ParsedExpression {
    let children = fragment.children.iter().map(parse_jsx_child).collect();

    ParsedExpression::JsxFragment {
        children,
        span: Some(text_span_from_oxc_span(fragment.span)),
    }
}

/// Returns `(tag_name, tag_name_span, component_name, component_span)`. The
/// component name/span are populated only when the tag is a value reference (a
/// capitalized component or a `Foo.Bar` member tag) so the checker can resolve it
/// and report TS2304 for missing names; intrinsic lowercase tags carry `None`.
fn parse_jsx_element_name(
    name: &JSXElementName<'_>,
) -> (String, Option<TextSpan>, Option<String>, Option<TextSpan>) {
    match name {
        JSXElementName::Identifier(identifier) => {
            // Intrinsic element such as `<div />`; not a value reference.
            let span = Some(text_span_from_oxc_span(identifier.span));
            (identifier.name.to_string(), span, None, None)
        }
        JSXElementName::IdentifierReference(identifier) => {
            // Component reference such as `<Button />`.
            let span = Some(text_span_from_oxc_span(identifier.span));
            (
                identifier.name.to_string(),
                span,
                Some(identifier.name.to_string()),
                span,
            )
        }
        JSXElementName::MemberExpression(member) => {
            let span = Some(text_span_from_oxc_span(member.span));
            let (head_name, head_span) = jsx_member_expression_head(member);
            (
                jsx_member_expression_name(member),
                span,
                head_name,
                head_span,
            )
        }
        JSXElementName::NamespacedName(namespaced) => {
            let span = Some(text_span_from_oxc_span(namespaced.span));
            (
                format!("{}:{}", namespaced.namespace.name, namespaced.name.name),
                span,
                None,
                None,
            )
        }
        JSXElementName::ThisExpression(this) => (
            "this".to_string(),
            Some(text_span_from_oxc_span(this.span)),
            None,
            None,
        ),
    }
}

/// Builds the dotted display name for a member tag (`UI.Button`, `A.B.C`).
fn jsx_member_expression_name(member: &JSXMemberExpression<'_>) -> String {
    let object = match &member.object {
        JSXMemberExpressionObject::IdentifierReference(identifier) => identifier.name.to_string(),
        JSXMemberExpressionObject::MemberExpression(inner) => jsx_member_expression_name(inner),
        JSXMemberExpressionObject::ThisExpression(_) => "this".to_string(),
    };
    format!("{}.{}", object, member.property.name)
}

/// Returns the head identifier of a member tag (the value that must resolve in
/// scope). `<UI.Button />` resolves `UI`; `<this.Foo />` has no resolvable head.
fn jsx_member_expression_head(
    member: &JSXMemberExpression<'_>,
) -> (Option<String>, Option<TextSpan>) {
    let mut object = &member.object;
    loop {
        match object {
            JSXMemberExpressionObject::IdentifierReference(identifier) => {
                return (
                    Some(identifier.name.to_string()),
                    Some(text_span_from_oxc_span(identifier.span)),
                );
            }
            JSXMemberExpressionObject::MemberExpression(inner) => {
                object = &inner.object;
            }
            JSXMemberExpressionObject::ThisExpression(_) => return (None, None),
        }
    }
}

fn parse_jsx_attribute_item(item: &JSXAttributeItem<'_>) -> Option<ParsedJsxAttribute> {
    match item {
        JSXAttributeItem::Attribute(attribute) => {
            let (name, name_span) = parse_jsx_attribute_name(&attribute.name);
            let (value, value_span) = match &attribute.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    parse_jsx_container_expression(container)
                }
                Some(JSXAttributeValue::Element(element)) => {
                    let span = Some(text_span_from_oxc_span(element.span));
                    (Some(parse_jsx_element(element)), span)
                }
                Some(JSXAttributeValue::Fragment(fragment)) => {
                    let span = Some(text_span_from_oxc_span(fragment.span));
                    (Some(parse_jsx_fragment(fragment)), span)
                }
                // String-literal values and boolean shorthand have nothing to check.
                Some(JSXAttributeValue::StringLiteral(_)) | None => (None, None),
            };
            Some(ParsedJsxAttribute {
                name,
                name_span,
                value,
                value_span,
            })
        }
        JSXAttributeItem::SpreadAttribute(spread) => {
            // Spread checking is out of scope, but the argument is still walked so
            // ordinary diagnostics inside it (e.g. unresolved names) are preserved.
            let (expression, span) = parse_expression(&spread.argument);
            Some(ParsedJsxAttribute {
                name: String::new(),
                name_span: None,
                value: Some(expression),
                value_span: Some(text_span_from_oxc_span(span)),
            })
        }
    }
}

fn parse_jsx_attribute_name(name: &JSXAttributeName<'_>) -> (String, Option<TextSpan>) {
    match name {
        JSXAttributeName::Identifier(identifier) => (
            identifier.name.to_string(),
            Some(text_span_from_oxc_span(identifier.span)),
        ),
        JSXAttributeName::NamespacedName(namespaced) => (
            format!("{}:{}", namespaced.namespace.name, namespaced.name.name),
            Some(text_span_from_oxc_span(namespaced.span)),
        ),
    }
}

fn parse_jsx_container_expression(
    container: &JSXExpressionContainer<'_>,
) -> (Option<ParsedExpression>, Option<TextSpan>) {
    match container.expression.as_expression() {
        Some(expression) => {
            let (parsed, span) = parse_expression(expression);
            (Some(parsed), Some(text_span_from_oxc_span(span)))
        }
        // Empty container `{}`: nothing to check.
        None => (None, Some(text_span_from_oxc_span(container.span))),
    }
}

fn parse_jsx_child(child: &JSXChild<'_>) -> ParsedJsxChild {
    match child {
        JSXChild::Text(_) => ParsedJsxChild::Text,
        JSXChild::Element(element) => ParsedJsxChild::Element(parse_jsx_element(element)),
        JSXChild::Fragment(fragment) => ParsedJsxChild::Element(parse_jsx_fragment(fragment)),
        JSXChild::ExpressionContainer(container) => {
            let (expression, span) = parse_jsx_container_expression(container);
            ParsedJsxChild::Expression { expression, span }
        }
        JSXChild::Spread(spread) => {
            let (expression, span) = parse_expression(&spread.expression);
            ParsedJsxChild::Expression {
                expression: Some(expression),
                span: Some(text_span_from_oxc_span(span)),
            }
        }
    }
}

pub(crate) fn parse_call_expression(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
) -> Option<ParsedCall> {
    let call = parse_call_expression_parts(call_expression)?;

    Some(ParsedCall {
        callee_name: call.callee_name,
        callee_span: call.callee_span,
        span: call.span,
        type_arguments: call.type_arguments,
        arguments: call.arguments,
    })
}

fn parse_call_expression_expression(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
) -> Option<ParsedExpression> {
    let arguments = call_expression
        .arguments
        .iter()
        .map(parse_call_argument)
        .collect::<Vec<_>>();
    let type_arguments = parse_call_type_arguments(call_expression)?;

    if call_expression.optional {
        match &call_expression.callee {
            Expression::StaticMemberExpression(member_expression) => {
                let (object, object_span) = parse_expression(&member_expression.object);
                return Some(ParsedExpression::OptionalPropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                });
            }
            _ => {
                let (callee, callee_span) = parse_expression(&call_expression.callee);
                return Some(ParsedExpression::OptionalCall {
                    callee: Box::new(callee),
                    callee_span: Some(text_span_from_oxc_span(callee_span)),
                    type_arguments,
                    arguments,
                });
            }
        }
    }

    match &call_expression.callee {
        Expression::Identifier(callee) => Some(ParsedExpression::Call {
            callee_name: callee.name.to_string(),
            callee_span: Some(text_span_from_oxc_span(callee.span)),
            type_arguments,
            arguments,
        }),
        Expression::StaticMemberExpression(member_expression) => {
            let (object, object_span) = parse_expression(&member_expression.object);

            if member_expression.optional {
                Some(ParsedExpression::OptionalPropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                })
            } else {
                Some(ParsedExpression::PropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                })
            }
        }
        _ => None,
    }
}

struct ParsedCallExpressionParts {
    callee_name: String,
    callee_span: Option<TextSpan>,
    span: Option<TextSpan>,
    type_arguments: Vec<crate::ParsedType>,
    arguments: Vec<ParsedCallArgument>,
}

fn parse_call_expression_parts(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
) -> Option<ParsedCallExpressionParts> {
    let Expression::Identifier(callee) = &call_expression.callee else {
        return None;
    };

    let arguments = call_expression
        .arguments
        .iter()
        .map(parse_call_argument)
        .collect();
    let type_arguments = parse_call_type_arguments(call_expression)?;

    Some(ParsedCallExpressionParts {
        callee_name: callee.name.to_string(),
        callee_span: Some(text_span_from_oxc_span(callee.span)),
        span: Some(text_span_from_oxc_span(call_expression.span)),
        type_arguments,
        arguments,
    })
}

fn parse_call_type_arguments(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
) -> Option<Vec<crate::ParsedType>> {
    match call_expression.type_arguments.as_deref() {
        Some(type_arguments) => parse_type_arguments(type_arguments),
        None => Some(Vec::new()),
    }
}

fn parse_new_expression(new_expression: &NewExpression<'_>) -> Option<ParsedExpression> {
    let arguments = new_expression
        .arguments
        .iter()
        .map(parse_call_argument)
        .collect::<Vec<_>>();
    let type_arguments = match new_expression.type_arguments.as_deref() {
        Some(type_arguments) => parse_type_arguments(type_arguments)?,
        None => Vec::new(),
    };
    let (callee, callee_span) = parse_expression(&new_expression.callee);

    Some(ParsedExpression::New {
        callee: Box::new(callee),
        callee_span: Some(text_span_from_oxc_span(callee_span)),
        type_arguments,
        arguments,
    })
}

fn parse_instantiation_expression(
    instantiation_expression: &oxc_ast::ast::TSInstantiationExpression<'_>,
) -> Option<ParsedExpression> {
    let type_arguments = parse_type_arguments(&instantiation_expression.type_arguments)?;

    match &instantiation_expression.expression {
        Expression::CallExpression(call_expression) => {
            parse_call_expression_expression_with_type_arguments(call_expression, type_arguments)
        }
        Expression::StaticMemberExpression(member_expression) => {
            parse_property_call_expression_with_type_arguments(member_expression, type_arguments)
        }
        Expression::Identifier(identifier) => Some(ParsedExpression::Call {
            callee_name: identifier.name.to_string(),
            callee_span: Some(text_span_from_oxc_span(identifier.span)),
            type_arguments,
            arguments: Vec::new(),
        }),
        _ => None,
    }
}

fn parse_call_expression_expression_with_type_arguments(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
    type_arguments: Vec<crate::ParsedType>,
) -> Option<ParsedExpression> {
    let arguments = call_expression
        .arguments
        .iter()
        .map(parse_call_argument)
        .collect::<Vec<_>>();

    if call_expression.optional {
        match &call_expression.callee {
            Expression::StaticMemberExpression(member_expression) => {
                let (object, object_span) = parse_expression(&member_expression.object);
                return Some(ParsedExpression::OptionalPropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                });
            }
            _ => {
                let (callee, callee_span) = parse_expression(&call_expression.callee);
                return Some(ParsedExpression::OptionalCall {
                    callee: Box::new(callee),
                    callee_span: Some(text_span_from_oxc_span(callee_span)),
                    type_arguments,
                    arguments,
                });
            }
        }
    }

    match &call_expression.callee {
        Expression::Identifier(callee) => Some(ParsedExpression::Call {
            callee_name: callee.name.to_string(),
            callee_span: Some(text_span_from_oxc_span(callee.span)),
            type_arguments,
            arguments,
        }),
        Expression::StaticMemberExpression(member_expression) => {
            let (object, object_span) = parse_expression(&member_expression.object);

            if member_expression.optional {
                Some(ParsedExpression::OptionalPropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                })
            } else {
                Some(ParsedExpression::PropertyCall {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name: member_expression.property.name.to_string(),
                    property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
                    call_span: Some(text_span_from_oxc_span(call_expression.span)),
                    type_arguments,
                    arguments,
                })
            }
        }
        _ => None,
    }
}

fn parse_property_call_expression_with_type_arguments(
    member_expression: &StaticMemberExpression<'_>,
    type_arguments: Vec<crate::ParsedType>,
) -> Option<ParsedExpression> {
    let (object, object_span) = parse_expression(&member_expression.object);
    Some(ParsedExpression::PropertyCall {
        object: Box::new(object),
        object_span: Some(text_span_from_oxc_span(object_span)),
        property_name: member_expression.property.name.to_string(),
        property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
        call_span: None,
        type_arguments,
        arguments: Vec::new(),
    })
}

fn parse_call_argument(argument: &Argument<'_>) -> ParsedCallArgument {
    let (expression, span) = match argument {
        Argument::SpreadElement(_) => (ParsedExpression::Unknown, argument.span()),
        Argument::BooleanLiteral(boolean_literal) => (
            ParsedExpression::BooleanLiteral(boolean_literal.value),
            argument.span(),
        ),
        Argument::NullLiteral(_) => (ParsedExpression::NullLiteral, argument.span()),
        Argument::NumericLiteral(numeric_literal) => (
            ParsedExpression::NumberLiteral(numeric_literal.value.to_string()),
            argument.span(),
        ),
        Argument::StringLiteral(string_literal) => (
            ParsedExpression::StringLiteral(string_literal.value.to_string()),
            argument.span(),
        ),
        Argument::Identifier(identifier) => (
            if identifier.name == "undefined" {
                ParsedExpression::UndefinedLiteral
            } else {
                ParsedExpression::Identifier {
                    name: identifier.name.to_string(),
                    span: Some(text_span_from_oxc_span(identifier.span)),
                }
            },
            argument.span(),
        ),
        Argument::BinaryExpression(binary_expression) => (
            parse_binary_expression(binary_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::LogicalExpression(logical_expression) => (
            parse_logical_expression(logical_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::UnaryExpression(unary_expression) => (
            parse_unary_expression(unary_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ParenthesizedExpression(parenthesized_expression) => (
            parse_expression(&parenthesized_expression.expression).0,
            argument.span(),
        ),
        Argument::AwaitExpression(await_expression) => (
            parse_expression(&await_expression.argument).0,
            argument.span(),
        ),
        Argument::ConditionalExpression(conditional_expression) => (
            parse_conditional_expression(conditional_expression)
                .unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ObjectExpression(object_expression) => (
            ParsedExpression::ObjectLiteral {
                properties: parse_object_properties(object_expression),
                span: Some(text_span_from_oxc_span(object_expression.span())),
            },
            argument.span(),
        ),
        Argument::ArrayExpression(array_expression) => (
            parse_array_expression(array_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::CallExpression(call_expression) => (
            parse_call_expression_expression(call_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ArrowFunctionExpression(arrow_expression) => (
            parse_arrow_function_expression(arrow_expression)
                .map(|arrow| ParsedExpression::ArrowFunction(Box::new(arrow)))
                .unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::StaticMemberExpression(member_expression) => (
            parse_static_member_expression(member_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ComputedMemberExpression(member_expression) => (
            parse_computed_member_expression(member_expression)
                .unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::TSAsExpression(as_expression) => {
            let (expression, expression_span) = parse_expression(&as_expression.expression);
            let ty = crate::parser::types::parse_type(&as_expression.type_annotation)
                .unwrap_or(crate::ParsedType::Unknown);

            if let crate::ParsedType::Named(named_type) = &ty {
                if named_type.name == "const" && named_type.type_arguments.is_empty() {
                    return ParsedCallArgument {
                        expression: ParsedExpression::ConstAssertion {
                            expression: Box::new(expression),
                            span: Some(text_span_from_oxc_span(as_expression.span)),
                        },
                        span: Some(text_span_from_oxc_span(as_expression.span)),
                    };
                }
            }

            (
                ParsedExpression::TypeAssertion {
                    expression: Box::new(expression),
                    expression_span: Some(text_span_from_oxc_span(expression_span)),
                    ty,
                    type_span: Some(text_span_from_oxc_span(
                        as_expression.type_annotation.span(),
                    )),
                },
                as_expression.span,
            )
        }
        Argument::TSSatisfiesExpression(satisfies_expression) => {
            let (expression, expression_span) = parse_expression(&satisfies_expression.expression);
            let ty = crate::parser::types::parse_type(&satisfies_expression.type_annotation)
                .unwrap_or(crate::ParsedType::Unknown);
            (
                ParsedExpression::SatisfiesExpression {
                    expression: Box::new(expression),
                    span: Some(text_span_from_oxc_span(expression_span)),
                    target_type: ty,
                    target_span: Some(text_span_from_oxc_span(
                        satisfies_expression.type_annotation.span(),
                    )),
                },
                satisfies_expression.span,
            )
        }
        Argument::TSNonNullExpression(non_null_expression) => {
            let (expression, _expression_span) = parse_expression(&non_null_expression.expression);
            (
                ParsedExpression::NonNullAssertion {
                    expression: Box::new(expression),
                    span: Some(text_span_from_oxc_span(non_null_expression.span)),
                    in_optional_chain: false,
                },
                non_null_expression.span,
            )
        }
        _ => (ParsedExpression::Unknown, argument.span()),
    };

    ParsedCallArgument {
        expression,
        span: Some(text_span_from_oxc_span(span)),
    }
}

fn parse_arrow_function_expression(
    arrow_expression: &ArrowFunctionExpression<'_>,
) -> Option<ParsedArrowFunction> {
    let parameters = arrow_expression
        .params
        .items
        .iter()
        .filter_map(parse_function_parameter)
        .collect::<Vec<_>>();
    let return_type = arrow_expression
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));

    let body = if let Some(expression) = arrow_expression.get_expression() {
        let (expression, _) = parse_expression(expression);
        ParsedArrowFunctionBody::Expression(Box::new(expression))
    } else {
        ParsedArrowFunctionBody::Block(parse_statement_list_as_function_body(
            &arrow_expression.body.statements,
        ))
    };

    Some(ParsedArrowFunction {
        type_parameters: parse_type_parameters(arrow_expression.type_parameters.as_deref()),
        parameters,
        return_type,
        is_async: arrow_expression.r#async,
        body,
        span: Some(text_span_from_oxc_span(arrow_expression.span)),
    })
}

fn parse_binary_expression(binary_expression: &BinaryExpression<'_>) -> Option<ParsedExpression> {
    let operator = match binary_expression.operator {
        BinaryOperator::StrictEquality => ParsedBinaryOperator::StrictEquals,
        BinaryOperator::StrictInequality => ParsedBinaryOperator::StrictNotEquals,
        BinaryOperator::Equality => ParsedBinaryOperator::Equals,
        BinaryOperator::Inequality => ParsedBinaryOperator::NotEquals,
        BinaryOperator::LessThan => ParsedBinaryOperator::LessThan,
        BinaryOperator::LessEqualThan => ParsedBinaryOperator::LessThanEquals,
        BinaryOperator::GreaterThan => ParsedBinaryOperator::GreaterThan,
        BinaryOperator::GreaterEqualThan => ParsedBinaryOperator::GreaterThanEquals,
        BinaryOperator::Addition => ParsedBinaryOperator::Add,
        BinaryOperator::Subtraction => ParsedBinaryOperator::Subtract,
        BinaryOperator::Multiplication => ParsedBinaryOperator::Multiply,
        BinaryOperator::Division => ParsedBinaryOperator::Divide,
        BinaryOperator::Remainder => ParsedBinaryOperator::Remainder,
        BinaryOperator::Exponential => ParsedBinaryOperator::Exponential,
        BinaryOperator::ShiftLeft => ParsedBinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight => ParsedBinaryOperator::ShiftRight,
        BinaryOperator::ShiftRightZeroFill => ParsedBinaryOperator::ShiftRightZeroFill,
        BinaryOperator::BitwiseOR => ParsedBinaryOperator::BitwiseOR,
        BinaryOperator::BitwiseXOR => ParsedBinaryOperator::BitwiseXOR,
        BinaryOperator::BitwiseAnd => ParsedBinaryOperator::BitwiseAnd,
        _ => return None,
    };

    let (left, left_span) = parse_expression(&binary_expression.left);
    let (right, right_span) = parse_expression(&binary_expression.right);

    Some(ParsedExpression::Binary {
        left: Box::new(left),
        left_span: Some(text_span_from_oxc_span(left_span)),
        operator,
        right: Box::new(right),
        right_span: Some(text_span_from_oxc_span(right_span)),
        operator_span: None,
    })
}

fn parse_logical_expression(
    logical_expression: &LogicalExpression<'_>,
) -> Option<ParsedExpression> {
    let (left, left_span) = parse_expression(&logical_expression.left);
    let (right, right_span) = parse_expression(&logical_expression.right);

    if logical_expression.operator == LogicalOperator::Coalesce {
        return Some(ParsedExpression::NullishCoalescing {
            left: Box::new(left),
            left_span: Some(text_span_from_oxc_span(left_span)),
            right: Box::new(right),
            right_span: Some(text_span_from_oxc_span(right_span)),
        });
    }

    let operator = match logical_expression.operator {
        LogicalOperator::And => ParsedLogicalOperator::And,
        LogicalOperator::Or => ParsedLogicalOperator::Or,
        LogicalOperator::Coalesce => unreachable!(),
    };

    Some(ParsedExpression::Logical {
        left: Box::new(left),
        left_span: Some(text_span_from_oxc_span(left_span)),
        operator,
        right: Box::new(right),
        right_span: Some(text_span_from_oxc_span(right_span)),
        operator_span: None,
    })
}

pub(crate) fn parse_conditional_expression(
    conditional_expression: &ConditionalExpression<'_>,
) -> Option<ParsedExpression> {
    let (condition, condition_span) = parse_expression(&conditional_expression.test);
    let (when_true, when_true_span) = parse_expression(&conditional_expression.consequent);
    let (when_false, when_false_span) = parse_expression(&conditional_expression.alternate);

    Some(ParsedExpression::Conditional {
        condition: Box::new(condition),
        condition_span: Some(text_span_from_oxc_span(condition_span)),
        when_true: Box::new(when_true),
        when_true_span: Some(text_span_from_oxc_span(when_true_span)),
        when_false: Box::new(when_false),
        when_false_span: Some(text_span_from_oxc_span(when_false_span)),
    })
}

pub(crate) fn parse_unary_expression(
    unary_expression: &UnaryExpression<'_>,
) -> Option<ParsedExpression> {
    let operator = match unary_expression.operator {
        UnaryOperator::LogicalNot => ParsedUnaryOperator::Not,
        UnaryOperator::UnaryPlus => ParsedUnaryOperator::Plus,
        UnaryOperator::UnaryNegation => ParsedUnaryOperator::Minus,
        UnaryOperator::BitwiseNot
        | UnaryOperator::Typeof
        | UnaryOperator::Void
        | UnaryOperator::Delete => {
            return Some(ParsedExpression::Unknown);
        }
    };

    let (operand, operand_span) = parse_expression(&unary_expression.argument);

    Some(ParsedExpression::Unary {
        operator,
        operator_span: None,
        operand: Box::new(operand),
        operand_span: Some(text_span_from_oxc_span(operand_span)),
    })
}

pub(crate) fn parse_object_properties(
    object_expression: &ObjectExpression<'_>,
) -> Vec<ParsedObjectProperty> {
    object_expression
        .properties
        .iter()
        .filter_map(|property_kind| {
            let ObjectPropertyKind::ObjectProperty(property) = property_kind else {
                return None;
            };

            let PropertyKey::StaticIdentifier(key) = &property.key else {
                return None;
            };

            if property.kind != PropertyKind::Init || property.computed {
                return None;
            }

            if property.method {
                return parse_object_method_shorthand(key, property);
            }

            let (value, value_span) = parse_expression(&property.value);

            Some(ParsedObjectProperty {
                name: key.name.to_string(),
                name_span: Some(text_span_from_oxc_span(key.span)),
                value,
                value_span: Some(text_span_from_oxc_span(value_span)),
                span: Some(text_span_from_oxc_span(property.span)),
                is_method: false,
            })
        })
        .collect()
}

/// Lowers object literal method shorthand (`{ foo(arg): R { ... } }`) into a property whose
/// value is an arrow function, so it reuses the existing arrow-function checking path while
/// honoring the declared parameter and return types.
fn parse_object_method_shorthand(
    key: &oxc_ast::ast::IdentifierName<'_>,
    property: &oxc_ast::ast::ObjectProperty<'_>,
) -> Option<ParsedObjectProperty> {
    let Expression::FunctionExpression(function) = &property.value else {
        return None;
    };

    let parameters = function
        .params
        .items
        .iter()
        .filter_map(parse_function_parameter)
        .collect::<Vec<_>>();

    let return_type = function
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));

    let body = function
        .body
        .as_ref()
        .map(|body| parse_statement_list_as_function_body(&body.statements))
        .unwrap_or_default();

    let arrow = ParsedArrowFunction {
        type_parameters: parse_type_parameters(function.type_parameters.as_deref()),
        parameters,
        return_type,
        is_async: function.r#async,
        body: ParsedArrowFunctionBody::Block(body),
        span: Some(text_span_from_oxc_span(function.span)),
    };

    Some(ParsedObjectProperty {
        name: key.name.to_string(),
        name_span: Some(text_span_from_oxc_span(key.span)),
        value: ParsedExpression::ArrowFunction(Box::new(arrow)),
        value_span: Some(text_span_from_oxc_span(function.span)),
        span: Some(text_span_from_oxc_span(property.span)),
        is_method: true,
    })
}

pub(crate) fn parse_array_expression(
    array_expression: &ArrayExpression<'_>,
) -> Option<ParsedExpression> {
    let mut elements = Vec::new();

    for element in &array_expression.elements {
        if element.is_spread() {
            return None;
        }

        if element.is_elision() {
            continue;
        }

        let Some(expression) = element.as_expression() else {
            return None;
        };

        let (parsed_expression, span) = parse_expression(expression);
        elements.push(crate::ParsedArrayElement {
            expression: parsed_expression,
            span: Some(text_span_from_oxc_span(span)),
        });
    }

    Some(ParsedExpression::ArrayLiteral {
        elements,
        span: Some(text_span_from_oxc_span(array_expression.span())),
    })
}

pub(crate) fn parse_static_member_expression(
    member_expression: &StaticMemberExpression<'_>,
) -> Option<ParsedExpression> {
    if member_expression.optional {
        let (object, object_span) = parse_expression(&member_expression.object);
        return Some(ParsedExpression::OptionalPropertyAccess {
            object: Box::new(object),
            object_span: Some(text_span_from_oxc_span(object_span)),
            property_name: member_expression.property.name.to_string(),
            property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
        });
    }

    let (object, object_span) = parse_expression(&member_expression.object);
    Some(ParsedExpression::PropertyAccess {
        object: Box::new(object),
        object_span: Some(text_span_from_oxc_span(object_span)),
        property_name: member_expression.property.name.to_string(),
        property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
    })
}

fn set_in_optional_chain(expr: &mut ParsedExpression) {
    match expr {
        ParsedExpression::NonNullAssertion {
            in_optional_chain,
            expression,
            ..
        } => {
            *in_optional_chain = true;
            set_in_optional_chain(expression);
        }
        ParsedExpression::PropertyAccess { object, .. } => set_in_optional_chain(object),
        ParsedExpression::OptionalPropertyAccess { object, .. } => set_in_optional_chain(object),
        ParsedExpression::OptionalIndexAccess { object, .. } => set_in_optional_chain(object),
        ParsedExpression::PropertyCall { object, .. } => set_in_optional_chain(object),
        ParsedExpression::OptionalPropertyCall { object, .. } => set_in_optional_chain(object),
        ParsedExpression::OptionalCall { callee, .. } => set_in_optional_chain(callee),
        _ => {}
    }
}

fn parse_chain_expression(chain_expression: &ChainExpression<'_>) -> Option<ParsedExpression> {
    let mut parsed = match &chain_expression.expression {
        ChainElement::CallExpression(call_expression) => {
            parse_call_expression_expression(call_expression)
        }
        ChainElement::StaticMemberExpression(member_expression) => {
            parse_static_member_expression(member_expression)
        }
        ChainElement::ComputedMemberExpression(member_expression) => {
            parse_computed_member_expression(member_expression)
        }
        ChainElement::TSNonNullExpression(non_null_expression) => {
            let (expression, _expression_span) = parse_expression(&non_null_expression.expression);
            Some(ParsedExpression::NonNullAssertion {
                expression: Box::new(expression),
                span: Some(text_span_from_oxc_span(non_null_expression.span)),
                in_optional_chain: true,
            })
        }
        _ => None,
    };

    if let Some(ref mut expr) = parsed {
        set_in_optional_chain(expr);
    }

    parsed
}

fn parse_computed_member_expression(
    member_expression: &ComputedMemberExpression<'_>,
) -> Option<ParsedExpression> {
    // String-literal bracket access (`obj["key"]`, `obj?.["key"]`) lowers to the
    // same (optional) property-access nodes as dot access so it reuses identical
    // property-lookup, optional-widening, and missing-property behavior.
    //
    // Purely numeric keys (e.g. `arr["0"]`) are left on the index-access path so
    // existing array/tuple numeric-index behavior is preserved unchanged.
    if let Expression::StringLiteral(string_literal) = &member_expression.expression {
        let key = string_literal.value.as_str();
        let is_numeric_index = !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit());

        if !is_numeric_index {
            let (object, object_span) = parse_expression(&member_expression.object);
            let property_name = string_literal.value.to_string();
            let property_span = Some(text_span_from_oxc_span(string_literal.span));

            if member_expression.optional {
                return Some(ParsedExpression::OptionalPropertyAccess {
                    object: Box::new(object),
                    object_span: Some(text_span_from_oxc_span(object_span)),
                    property_name,
                    property_span,
                });
            }

            return Some(ParsedExpression::PropertyAccess {
                object: Box::new(object),
                object_span: Some(text_span_from_oxc_span(object_span)),
                property_name,
                property_span,
            });
        }
    }

    if member_expression.optional {
        let (object, object_span) = parse_expression(&member_expression.object);
        let (index, index_span) = parse_expression(&member_expression.expression);
        return Some(ParsedExpression::OptionalIndexAccess {
            object: Box::new(object),
            object_span: Some(text_span_from_oxc_span(object_span)),
            index: Box::new(index),
            index_span: Some(text_span_from_oxc_span(index_span)),
        });
    }

    let Expression::Identifier(object_identifier) = &member_expression.object else {
        return None;
    };

    let (index, index_span) = parse_expression(&member_expression.expression);

    Some(ParsedExpression::IndexAccess {
        object_name: object_identifier.name.to_string(),
        object_span: Some(text_span_from_oxc_span(object_identifier.span)),
        index: Box::new(index),
        index_span: Some(text_span_from_oxc_span(index_span)),
    })
}
