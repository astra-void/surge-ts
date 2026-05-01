use crate::{FunctionType, ObjectType, UnionType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLiteralType {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Number,
    Boolean,
    Undefined,
    Void,
    Any,
    Unknown,
    StringLiteral(String),
    NumberLiteral(NumberLiteralType),
    BooleanLiteral(bool),
    Function(FunctionType),
    Object(ObjectType),
    Union(UnionType),
}

impl Type {
    pub fn base_primitive(&self) -> Option<Type> {
        match self {
            Type::String | Type::StringLiteral(_) => Some(Type::String),
            Type::Number | Type::NumberLiteral(_) => Some(Type::Number),
            Type::Boolean | Type::BooleanLiteral(_) => Some(Type::Boolean),
            _ => None,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Type::String => "string".to_string(),
            Type::Number => "number".to_string(),
            Type::Boolean => "boolean".to_string(),
            Type::Undefined => "undefined".to_string(),
            Type::Void => "void".to_string(),
            Type::Any => "any".to_string(),
            Type::Unknown => "unknown".to_string(),
            Type::StringLiteral(value) => format!("{value:?}"),
            Type::NumberLiteral(value) => value.value.clone(),
            Type::BooleanLiteral(value) => value.to_string(),
            Type::Function(function) => function.name(),
            Type::Object(object) => {
                let properties = object
                    .properties
                    .iter()
                    .map(|(name, property)| {
                        if property.is_optional() {
                            format!("{name}?: {}", property.ty.name())
                        } else {
                            format!("{name}: {}", property.ty.name())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ");

                if properties.is_empty() {
                    "{}".to_string()
                } else {
                    format!("{{ {}; }}", properties)
                }
            }
            Type::Union(union) => {
                if union.types.is_empty() {
                    return "unknown".to_string();
                }

                union
                    .types
                    .iter()
                    .map(Type::name)
                    .collect::<Vec<_>>()
                    .join(" | ")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_type_name_quotes_value() {
        assert_eq!(Type::StringLiteral("ok".to_string()).name(), r#""ok""#);
    }

    #[test]
    fn number_literal_type_name_is_stable() {
        assert_eq!(
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string()
            })
            .name(),
            "1"
        );
    }

    #[test]
    fn boolean_literal_type_name_is_true_false() {
        assert_eq!(Type::BooleanLiteral(true).name(), "true");
        assert_eq!(Type::BooleanLiteral(false).name(), "false");
    }

    #[test]
    fn void_type_name_is_void() {
        assert_eq!(Type::Void.name(), "void");
    }
}
