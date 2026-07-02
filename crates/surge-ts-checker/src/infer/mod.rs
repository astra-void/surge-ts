pub(crate) mod expression;
mod types;

pub(crate) use expression::{infer_expression, tuple_index_value};
pub(crate) use types::{
    TypeParameterSubstitution, map_parsed_type, map_parsed_type_with_substitution,
    report_duplicate_type_parameters, string_literal_union_keys,
    substitute_parsed_type_parameters_deep, validate_local_type_declaration,
};

use surge_ts_syntax::TextSpan;
use surge_ts_types::Type;

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
