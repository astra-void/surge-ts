use crate::{FunctionType, ObjectProperty, ObjectType, UnionType};

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
    Array(Box<Type>),
    Tuple(Vec<Type>),
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

    pub fn get_property_access_type(&self, name: &str) -> Option<Type> {
        match self {
            Type::Object(object) => object.get_property_access_type(name),
            Type::Array(element) => array_property_access_type(name, element.as_ref()),
            Type::String | Type::StringLiteral(_) => string_property_access_type(name),
            Type::Number | Type::NumberLiteral(_) => number_property_access_type(name),
            _ => None,
        }
    }

    pub fn builtin_constructor_result_type(name: &str) -> Option<Type> {
        match name {
            "Array" => Some(Type::Array(Box::new(Type::Any))),
            "Uint8Array" => Some(Type::Array(Box::new(Type::Number))),
            "Map" => Some(Type::Object(map_instance_type())),
            "Date" => Some(Type::Any),
            "TextEncoder" => Some(text_encoder_instance_type()),
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
                let mut parts = object
                    .properties
                    .iter()
                    .map(|(name, property)| {
                        if property.is_optional() {
                            format!("{name}?: {}", property.ty.name())
                        } else {
                            format!("{name}: {}", property.ty.name())
                        }
                    })
                    .collect::<Vec<_>>();

                if let Some(index_type) = &object.string_index_type {
                    parts.push(format!("[key: string]: {}", index_type.name()));
                }

                let properties = parts.join("; ");

                if properties.is_empty() {
                    "{}".to_string()
                } else {
                    format!("{{ {}; }}", properties)
                }
            }
            Type::Array(element) => format!("{}[]", array_element_name(element)),
            Type::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(Type::name)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{elements}]")
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

fn string_property_access_type(name: &str) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        "replace" => Some(Type::Function(FunctionType {
            parameters: vec![Type::String, Type::String],
            return_type: Box::new(Type::String),
            is_variadic: false,
            required_parameter_count: 2,
        })),
        "indexOf" => Some(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 1,
        })),
        "split" => Some(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Array(Box::new(Type::String))),
            is_variadic: true,
            required_parameter_count: 1,
        })),
        "slice" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::String),
            is_variadic: true,
            required_parameter_count: 1,
        })),
        "toLowerCase" | "toUpperCase" => Some(Type::Function(FunctionType {
            parameters: vec![],
            return_type: Box::new(Type::String),
            is_variadic: false,
            required_parameter_count: 0,
        })),
        "toString" => Some(Type::Function(FunctionType {
            parameters: vec![],
            return_type: Box::new(Type::String),
            is_variadic: false,
            required_parameter_count: 0,
        })),
        "padStart" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Number, Type::String],
            return_type: Box::new(Type::String),
            is_variadic: true,
            required_parameter_count: 1,
        })),
        "charCodeAt" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number),
            is_variadic: false,
            required_parameter_count: 1,
        })),
        _ => None,
    }
}

fn number_property_access_type(name: &str) -> Option<Type> {
    match name {
        "toString" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::String),
            is_variadic: true,
            required_parameter_count: 0,
        })),
        _ => None,
    }
}

fn array_property_access_type(name: &str, element: &Type) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        "map" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Function(FunctionType {
                parameters: vec![element.clone()],
                return_type: Box::new(Type::Any),
                is_variadic: false,
                required_parameter_count: 1,
            })],
            return_type: Box::new(Type::Array(Box::new(Type::Any))),
            is_variadic: false,
            required_parameter_count: 1,
        })),
        "find" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Function(FunctionType {
                parameters: vec![element.clone()],
                return_type: Box::new(Type::Boolean),
                is_variadic: false,
                required_parameter_count: 1,
            })],
            return_type: Box::new(Type::Union(UnionType {
                types: vec![element.clone(), Type::Undefined],
            })),
            is_variadic: false,
            required_parameter_count: 1,
        })),
        "filter" => Some(Type::Function(FunctionType {
            parameters: vec![Type::Function(FunctionType {
                parameters: vec![element.clone()],
                return_type: Box::new(Type::Boolean),
                is_variadic: false,
                required_parameter_count: 1,
            })],
            return_type: Box::new(Type::Array(Box::new(element.clone()))),
            is_variadic: false,
            required_parameter_count: 1,
        })),
        "join" => Some(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::String),
            is_variadic: true,
            required_parameter_count: 0,
        })),
        "push" => Some(Type::Function(FunctionType {
            parameters: vec![element.clone()],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 1,
        })),
        "includes" => Some(Type::Function(FunctionType {
            parameters: vec![element.clone()],
            return_type: Box::new(Type::Boolean),
            is_variadic: false,
            required_parameter_count: 1,
        })),
        _ => None,
    }
}

fn map_instance_type() -> ObjectType {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "get".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Any),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );
    properties.insert(
        "set".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any, Type::Any],
            return_type: Box::new(Type::Any),
            is_variadic: false,
            required_parameter_count: 2,
        })),
    );
    properties.insert(
        "has".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Boolean),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );
    properties.insert(
        "delete".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Boolean),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );
    properties.insert(
        "clear".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![],
            return_type: Box::new(Type::Void),
            is_variadic: false,
            required_parameter_count: 0,
        })),
    );
    properties.insert("size".to_string(), ObjectProperty::required(Type::Number));
    ObjectType {
        properties,
        string_index_type: None,
    }
}

fn text_encoder_instance_type() -> Type {
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "encode".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Array(Box::new(Type::Number))),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );

    Type::Object(ObjectType {
        properties,
        string_index_type: None,
    })
}

fn array_element_name(element: &Type) -> String {
    match element {
        Type::Union(_) | Type::Function(_) => format!("({})", element.name()),
        _ => element.name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectProperty;

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

    #[test]
    fn array_type_name_string() {
        assert_eq!(Type::Array(Box::new(Type::String)).name(), "string[]");
    }

    #[test]
    fn array_type_name_number() {
        assert_eq!(Type::Array(Box::new(Type::Number)).name(), "number[]");
    }

    #[test]
    fn array_type_name_boolean() {
        assert_eq!(Type::Array(Box::new(Type::Boolean)).name(), "boolean[]");
    }

    #[test]
    fn array_type_name_undefined() {
        assert_eq!(Type::Array(Box::new(Type::Undefined)).name(), "undefined[]");
    }

    #[test]
    fn array_type_name_void() {
        assert_eq!(Type::Array(Box::new(Type::Void)).name(), "void[]");
    }

    #[test]
    fn array_type_name_literal() {
        assert_eq!(
            Type::Array(Box::new(Type::StringLiteral("ok".to_string()))).name(),
            r#""ok"[]"#
        );
    }

    #[test]
    fn array_type_name_union() {
        assert_eq!(
            Type::Array(Box::new(Type::Union(crate::UnionType {
                types: vec![Type::String, Type::Number],
            })))
            .name(),
            "(string | number)[]"
        );
    }

    #[test]
    fn array_type_name_function() {
        assert_eq!(
            Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(Type::String),
                is_variadic: false,
                required_parameter_count: 0
            })))
            .name(),
            "(() => string)[]"
        );
    }

    #[test]
    fn array_type_name_object() {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Array(Box::new(Type::Object(ObjectType {
                properties,
                string_index_type: None,
            })))
            .name(),
            "{ name: string; }[]"
        );
    }

    #[test]
    fn array_type_name_nested_array() {
        assert_eq!(
            Type::Array(Box::new(Type::Array(Box::new(Type::String)))).name(),
            "string[][]"
        );
    }

    #[test]
    fn tuple_type_name_empty() {
        assert_eq!(Type::Tuple(vec![]).name(), "[]");
    }

    #[test]
    fn tuple_type_name_one_element() {
        assert_eq!(Type::Tuple(vec![Type::String]).name(), "[string]");
    }

    #[test]
    fn tuple_type_name_two_elements() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).name(),
            "[string, number]"
        );
    }

    #[test]
    fn tuple_type_name_literal_element() {
        assert_eq!(
            Type::Tuple(vec![Type::StringLiteral("ok".to_string()), Type::Number]).name(),
            r#"["ok", number]"#
        );
    }

    #[test]
    fn tuple_type_name_union_element() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Union(crate::UnionType {
                    types: vec![Type::String, Type::Number],
                }),
                Type::Boolean,
            ])
            .name(),
            "[string | number, boolean]"
        );
    }

    #[test]
    fn tuple_type_name_function_element() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Function(FunctionType {
                    parameters: vec![],
                    return_type: Box::new(Type::Void),
                    is_variadic: false,
                    required_parameter_count: 0
                }),
                Type::String,
            ])
            .name(),
            "[() => void, string]"
        );
    }

    #[test]
    fn tuple_type_name_object_element() {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Tuple(vec![
                Type::Object(ObjectType {
                    properties,
                    string_index_type: None,
                }),
                Type::Number,
            ])
            .name(),
            "[{ name: string; }, number]"
        );
    }

    #[test]
    fn tuple_type_name_array_element() {
        assert_eq!(
            Type::Tuple(vec![Type::Array(Box::new(Type::String)), Type::Number]).name(),
            "[string[], number]"
        );
    }

    #[test]
    fn tuple_type_name_nested_tuple() {
        assert_eq!(
            Type::Tuple(vec![
                Type::Tuple(vec![Type::String, Type::Number]),
                Type::Boolean,
            ])
            .name(),
            "[[string, number], boolean]"
        );
    }
}
