mod expression;
mod types;

pub(crate) use expression::infer_expression;
pub(crate) use types::{
    TypeParameterSubstitution, map_parsed_type, map_parsed_type_with_substitution,
    report_duplicate_type_parameters,
};

use typescript_rust_syntax::TextSpan;
use typescript_rust_types::Type;

#[derive(Debug, Clone)]
pub(crate) enum InferredExpression {
    Known(Type),
    UnresolvedIdentifier {
        name: String,
        span: Option<TextSpan>,
    },
    MissingProperty {
        property_name: String,
        object_type: Type,
        span: Option<TextSpan>,
    },
    Unknown,
}
