use crate::{ParsedFunctionType, ParsedFunctionTypeParameter};
use oxc_ast::ast::{
    BindingPattern, FormalParameter, FormalParameterRest, TSConstructorType, TSFunctionType,
    TSThisParameter,
};

use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_parameters};

/// Lowers a constructor type (`new (args) => T`, `abstract new (args) => T`) to a
/// plain callable signature. surge does not model newability separately, and the
/// only thing downstream inference needs from a constructor type is its parameter
/// and return shapes — so reusing [`ParsedFunctionType`] keeps a union member like
/// React's `JSXElementConstructor<P>` (`((props: P) => …) | (new (props: P) => …)`)
/// fully parsed instead of collapsing the whole union to `Unknown`.
pub(crate) fn parse_constructor_type(
    constructor_type: &TSConstructorType<'_>,
) -> Option<ParsedFunctionType> {
    let mut parameters = Vec::new();

    for parameter in &constructor_type.params.items {
        parameters.push(parse_function_type_parameter(parameter)?);
    }

    if let Some(rest) = constructor_type.params.rest.as_deref() {
        parameters.push(parse_function_type_rest_parameter(rest)?);
    }

    let return_type = parse_type_annotation(&constructor_type.return_type)?;

    Some(ParsedFunctionType {
        parameters,
        return_type: Box::new(return_type),
        type_parameters: parse_type_parameters(constructor_type.type_parameters.as_deref()),
    })
}

pub(crate) fn parse_function_type(
    function_type: &TSFunctionType<'_>,
) -> Option<ParsedFunctionType> {
    let mut parameters = Vec::new();

    // A leading `this: T` is a fake parameter: keep it parser-safe and stored as
    // metadata so later phases can exclude it from arity and argument matching.
    // A `this` without a type annotation carries no metadata, so it is dropped
    // rather than failing the whole signature.
    if let Some(this_parameter) = function_type
        .this_param
        .as_deref()
        .and_then(parse_this_parameter)
    {
        parameters.push(this_parameter);
    }

    for parameter in &function_type.params.items {
        parameters.push(parse_function_type_parameter(parameter)?);
    }

    if let Some(rest) = function_type.params.rest.as_deref() {
        parameters.push(parse_function_type_rest_parameter(rest)?);
    }

    let return_type = parse_type_annotation(&function_type.return_type)?;

    Some(ParsedFunctionType {
        parameters,
        return_type: Box::new(return_type),
        type_parameters: parse_type_parameters(function_type.type_parameters.as_deref()),
    })
}

fn parse_this_parameter(
    this_parameter: &TSThisParameter<'_>,
) -> Option<ParsedFunctionTypeParameter> {
    let ty = this_parameter
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedFunctionTypeParameter {
        name: Some("this".to_string()),
        name_span: Some(text_span_from_oxc_span(this_parameter.this_span)),
        ty,
        optional: false,
        is_this: true,
        rest: false,
    })
}

pub(crate) fn parse_function_type_rest_parameter(
    rest: &FormalParameterRest<'_>,
) -> Option<ParsedFunctionTypeParameter> {
    let BindingPattern::BindingIdentifier(binding) = &rest.rest.argument else {
        return None;
    };

    let ty = rest
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedFunctionTypeParameter {
        name: Some(binding.name.to_string()),
        name_span: Some(text_span_from_oxc_span(binding.span)),
        ty,
        optional: false,
        is_this: false,
        rest: true,
    })
}

pub(crate) fn parse_function_type_parameter(
    parameter: &FormalParameter<'_>,
) -> Option<ParsedFunctionTypeParameter> {
    // A destructuring-pattern parameter (`render: ({ field }: Props) => …`,
    // react-hook-form's `ControllerProps.render`) has no single name — only its
    // annotation matters for the signature. Failing here would degrade the whole
    // enclosing function type (and any intersection it sits in) to `Unknown`.
    let (name, name_span) = match &parameter.pattern {
        BindingPattern::BindingIdentifier(binding) => (
            Some(binding.name.to_string()),
            Some(text_span_from_oxc_span(binding.span)),
        ),
        _ => (None, None),
    };

    let ty = parameter
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation))?;

    Some(ParsedFunctionTypeParameter {
        name,
        name_span,
        ty,
        optional: parameter.optional || parameter.initializer.is_some(),
        is_this: false,
        rest: false,
    })
}
