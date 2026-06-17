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
    Never,
    StringLiteral(String),
    NumberLiteral(NumberLiteralType),
    BooleanLiteral(bool),
    Function(FunctionType),
    Object(ObjectType),
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Union(UnionType),
}

fn function_type(
    parameters: Vec<Type>,
    return_type: Type,
    is_variadic: bool,
    required_parameter_count: usize,
) -> Type {
    Type::Function(FunctionType::new(
        parameters,
        return_type,
        is_variadic,
        required_parameter_count,
    ))
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
            Type::Tuple(_) => tuple_property_access_type(name),
            Type::String | Type::StringLiteral(_) => string_property_access_type(name),
            Type::Number | Type::NumberLiteral(_) => number_property_access_type(name),
            _ => None,
        }
    }

    pub fn builtin_constructor_result_type(name: &str) -> Option<Type> {
        match name {
            "Date" => Some(Type::Any),
            "Array" => Some(Type::Array(Box::new(Type::Any))),
            "Uint8Array" => Some(Type::Array(Box::new(Type::Number))),
            "Map" => Some(Type::Object(ObjectType::new(
                {
                    let mut properties = std::collections::BTreeMap::new();
                    properties.insert(
                        "get".to_string(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Any,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "set".to_string(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any, Type::Any],
                            Type::Any,
                            false,
                            2,
                        )),
                    );
                    properties.insert(
                        "has".to_string(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Boolean,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "delete".to_string(),
                        crate::ObjectProperty::required(function_type(
                            vec![Type::Any],
                            Type::Boolean,
                            false,
                            1,
                        )),
                    );
                    properties.insert(
                        "clear".to_string(),
                        crate::ObjectProperty::required(function_type(
                            vec![],
                            Type::Void,
                            false,
                            0,
                        )),
                    );
                    properties.insert(
                        "size".to_string(),
                        crate::ObjectProperty::required(Type::Number),
                    );
                    properties
                },
                None,
            ))),
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
            Type::Never => "never".to_string(),
            Type::StringLiteral(value) => format!("{value:?}"),
            Type::NumberLiteral(value) => value.value.clone(),
            Type::BooleanLiteral(value) => value.to_string(),
            Type::Function(function) => function.name(),
            Type::Object(object) => {
                if let Some(alias_name) = &object.alias_name {
                    return alias_name.to_string();
                }

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
                if union.types().is_empty() {
                    return "unknown".to_string();
                }

                union
                    .types()
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
        "replace" => Some(function_type(
            vec![Type::String, Type::String],
            Type::String,
            false,
            2,
        )),
        "indexOf" => Some(function_type(vec![Type::String], Type::Number, true, 1)),
        "split" => Some(function_type(
            vec![Type::String],
            Type::Array(Box::new(Type::String)),
            true,
            1,
        )),
        "slice" => Some(function_type(vec![Type::Number], Type::String, true, 1)),
        "startsWith" | "endsWith" => {
            Some(function_type(vec![Type::String], Type::Boolean, true, 1))
        }
        "toLowerCase" | "toUpperCase" => Some(function_type(vec![], Type::String, false, 0)),
        "toString" => Some(function_type(vec![], Type::String, false, 0)),
        "padStart" => Some(function_type(
            vec![Type::Number, Type::String],
            Type::String,
            true,
            1,
        )),
        "charCodeAt" => Some(function_type(vec![Type::Number], Type::Number, false, 1)),
        _ => None,
    }
}

fn number_property_access_type(name: &str) -> Option<Type> {
    match name {
        "toString" => Some(function_type(vec![Type::Number], Type::String, true, 0)),
        _ => None,
    }
}

fn tuple_property_access_type(name: &str) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        _ => None,
    }
}

fn array_property_access_type(name: &str, element: &Type) -> Option<Type> {
    match name {
        "length" => Some(Type::Number),
        "map" => Some(function_type(
            vec![function_type(vec![element.clone()], Type::Any, false, 1)],
            Type::Array(Box::new(Type::Any)),
            false,
            1,
        )),
        "find" => Some(function_type(
            vec![function_type(
                vec![element.clone()],
                Type::Boolean,
                false,
                1,
            )],
            Type::Union(UnionType::new(vec![element.clone(), Type::Undefined])),
            false,
            1,
        )),
        "filter" => Some(function_type(
            vec![function_type(
                vec![element.clone()],
                Type::Boolean,
                false,
                1,
            )],
            Type::Array(Box::new(element.clone())),
            false,
            1,
        )),
        "join" => Some(function_type(vec![Type::String], Type::String, true, 0)),
        "push" => Some(function_type(vec![element.clone()], Type::Number, true, 1)),
        "includes" => Some(function_type(
            vec![element.clone()],
            Type::Boolean,
            false,
            1,
        )),
        _ => None,
    }
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
    fn array_length_property_is_number() {
        assert_eq!(
            Type::Array(Box::new(Type::String)).get_property_access_type("length"),
            Some(Type::Number)
        );
    }

    #[test]
    fn tuple_length_property_is_number() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).get_property_access_type("length"),
            Some(Type::Number)
        );
    }

    #[test]
    fn tuple_unknown_property_is_unsupported() {
        assert_eq!(
            Type::Tuple(vec![Type::String, Type::Number]).get_property_access_type("push"),
            None
        );
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
            Type::Array(Box::new(Type::Union(crate::UnionType::new(vec![
                Type::String,
                Type::Number,
            ]))))
            .name(),
            "(string | number)[]"
        );
    }

    #[test]
    fn array_type_name_function() {
        assert_eq!(
            Type::Array(Box::new(Type::Function(FunctionType::new(
                vec![],
                Type::String,
                false,
                0,
            ))))
            .name(),
            "(() => string)[]"
        );
    }

    #[test]
    fn array_type_name_object() {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Array(Box::new(Type::Object(ObjectType::new(properties, None)))).name(),
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
                Type::Union(crate::UnionType::new(vec![Type::String, Type::Number,])),
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
                Type::Function(FunctionType::new(vec![], Type::Void, false, 0)),
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
                Type::Object(ObjectType::new(properties, None)),
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
