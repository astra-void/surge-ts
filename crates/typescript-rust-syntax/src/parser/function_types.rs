use crate::{ParsedFunctionType, ParsedFunctionTypeParameter};
use oxc_ast::ast::{BindingPattern, FormalParameter, TSFunctionType};

use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_parameters};

pub(crate) fn parse_function_type(
    function_type: &TSFunctionType<'_>,
) -> Option<ParsedFunctionType> {
    if function_type.this_param.is_some() || function_type.params.rest.is_some() {
        return None;
    }

    let parameters = function_type
        .params
        .items
        .iter()
        .map(parse_function_type_parameter)
        .collect::<Option<Vec<_>>>()?;

    let return_type = parse_type_annotation(&function_type.return_type)?;

    Some(ParsedFunctionType {
        parameters,
        return_type: Box::new(return_type),
        type_parameters: parse_type_parameters(function_type.type_parameters.as_deref()),
    })
}

fn parse_function_type_parameter(
    parameter: &FormalParameter<'_>,
) -> Option<ParsedFunctionTypeParameter> {
    if parameter.optional || parameter.initializer.is_some() {
        return None;
    }

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
    })
}
