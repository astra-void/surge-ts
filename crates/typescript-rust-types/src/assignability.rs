use crate::{FunctionType, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectAssignabilityFailure {
    MissingProperty {
        property_name: String,
    },
    PropertyTypeMismatch {
        property_name: String,
        source_type: Type,
        target_type: Type,
    },
}

pub fn is_assignable_to(from: &Type, to: &Type) -> bool {
    if from == to
        || matches!(from, Type::Any)
        || matches!(to, Type::Any)
        || matches!(to, Type::Unknown)
    {
        return true;
    }

    if from
        .base_primitive()
        .as_ref()
        .is_some_and(|base| base == to)
    {
        return true;
    }

    match (from, to) {
        (Type::Undefined, Type::Void) => true,
        (Type::Function(source), Type::Function(target)) => {
            is_function_assignable_to(source, target)
        }
        (Type::Array(source), Type::Array(target)) => is_assignable_to(source, target),
        (Type::Tuple(source), Type::Tuple(target)) => {
            source.len() == target.len()
                && source
                    .iter()
                    .zip(target.iter())
                    .all(|(source_ty, target_ty)| is_assignable_to(source_ty, target_ty))
        }
        (Type::Tuple(source), Type::Array(target)) => source
            .iter()
            .all(|source_ty| is_assignable_to(source_ty, target)),
        (Type::Union(from_union), Type::Union(to_union)) => {
            from_union.types.iter().all(|from_ty| {
                to_union
                    .types
                    .iter()
                    .any(|to_ty| is_assignable_to(from_ty, to_ty))
            })
        }
        (Type::Union(from_union), to_ty) => from_union
            .types
            .iter()
            .all(|from_ty| is_assignable_to(from_ty, to_ty)),
        (from_ty, Type::Union(to_union)) => to_union
            .types
            .iter()
            .any(|to_ty| is_assignable_to(from_ty, to_ty)),
        (Type::Object(_), Type::Object(_)) => object_assignability_failure(from, to).is_none(),
        _ => false,
    }
}

fn is_function_assignable_to(source: &FunctionType, target: &FunctionType) -> bool {
    if source.parameters.len() != target.parameters.len() {
        return false;
    }

    let parameters_compatible = source.parameters.iter().zip(target.parameters.iter()).all(
        |(source_parameter, target_parameter)| {
            source_parameter == target_parameter
                || (is_assignable_to(source_parameter, target_parameter)
                    && is_assignable_to(target_parameter, source_parameter))
        },
    );

    parameters_compatible && is_assignable_to(&source.return_type, &target.return_type)
}

pub fn object_assignability_failure(
    source: &Type,
    target: &Type,
) -> Option<ObjectAssignabilityFailure> {
    let (Type::Object(source), Type::Object(target)) = (source, target) else {
        return None;
    };

    for (property_name, target_property) in &target.properties {
        let Some(source_property) = source.properties.get(property_name) else {
            if target_property.is_optional() {
                continue;
            }

            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.clone(),
            });
        };

        if source_property.is_optional() && target_property.is_required() {
            return Some(ObjectAssignabilityFailure::MissingProperty {
                property_name: property_name.clone(),
            });
        }

        if !is_assignable_to(&source_property.ty, &target_property.ty) {
            return Some(ObjectAssignabilityFailure::PropertyTypeMismatch {
                property_name: property_name.clone(),
                source_type: source_property.ty.clone(),
                target_type: target_property.ty.clone(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunctionType, NumberLiteralType, ObjectProperty, ObjectType, union_type};
    use std::collections::BTreeMap;

    #[test]
    fn string_literal_assignable_to_string() {
        assert!(is_assignable_to(
            &Type::StringLiteral("ok".to_string()),
            &Type::String
        ));
    }

    #[test]
    fn string_not_assignable_to_string_literal() {
        assert!(!is_assignable_to(
            &Type::String,
            &Type::StringLiteral("ok".to_string())
        ));
    }

    #[test]
    fn matching_string_literal_assignable_to_same_literal() {
        let literal = Type::StringLiteral("ok".to_string());
        assert!(is_assignable_to(&literal, &literal));
    }

    #[test]
    fn different_string_literal_not_assignable() {
        assert!(!is_assignable_to(
            &Type::StringLiteral("ok".to_string()),
            &Type::StringLiteral("no".to_string())
        ));
    }

    #[test]
    fn number_literal_assignability() {
        let literal = Type::NumberLiteral(NumberLiteralType {
            value: "1".to_string(),
        });

        assert!(is_assignable_to(&literal, &Type::Number));
        assert!(!is_assignable_to(
            &Type::Number,
            &Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            })
        ));
    }

    #[test]
    fn boolean_literal_assignability() {
        assert!(is_assignable_to(
            &Type::BooleanLiteral(true),
            &Type::Boolean
        ));
        assert!(!is_assignable_to(
            &Type::Boolean,
            &Type::BooleanLiteral(true)
        ));
        assert!(!is_assignable_to(
            &Type::BooleanLiteral(true),
            &Type::BooleanLiteral(false)
        ));
    }

    #[test]
    fn literal_union_assignability() {
        assert!(is_assignable_to(
            &union_type(vec![
                Type::StringLiteral("ok".to_string()),
                Type::StringLiteral("no".to_string())
            ]),
            &Type::String
        ));
        assert!(!is_assignable_to(
            &Type::String,
            &union_type(vec![
                Type::StringLiteral("ok".to_string()),
                Type::StringLiteral("no".to_string())
            ])
        ));
    }

    #[test]
    fn literal_object_assignability() {
        let mut source_properties = BTreeMap::new();
        source_properties.insert(
            "kind".to_string(),
            ObjectProperty::required(Type::StringLiteral("click".to_string())),
        );

        let mut target_properties = BTreeMap::new();
        target_properties.insert("kind".to_string(), ObjectProperty::required(Type::String));

        assert!(is_assignable_to(
            &Type::Object(ObjectType {
                properties: source_properties,
            }),
            &Type::Object(ObjectType {
                properties: target_properties,
            })
        ));
    }

    #[test]
    fn void_assignability() {
        assert!(is_assignable_to(&Type::Undefined, &Type::Void));
        assert!(is_assignable_to(&Type::Void, &Type::Any));
        assert!(is_assignable_to(&Type::Any, &Type::Void));
        assert!(!is_assignable_to(&Type::Void, &Type::Undefined));
    }

    #[test]
    fn undefined_assignable_to_void() {
        assert!(is_assignable_to(&Type::Undefined, &Type::Void));
    }

    #[test]
    fn void_assignable_to_void() {
        assert!(is_assignable_to(&Type::Void, &Type::Void));
    }

    #[test]
    fn any_assignable_to_void() {
        assert!(is_assignable_to(&Type::Any, &Type::Void));
    }

    #[test]
    fn void_assignable_to_any() {
        assert!(is_assignable_to(&Type::Void, &Type::Any));
    }

    #[test]
    fn void_not_assignable_to_undefined() {
        assert!(!is_assignable_to(&Type::Void, &Type::Undefined));
    }

    #[test]
    fn void_not_assignable_to_string() {
        assert!(!is_assignable_to(&Type::Void, &Type::String));
    }

    #[test]
    fn void_union_assignability() {
        let void_union = union_type(vec![Type::Void, Type::Undefined]);

        assert!(is_assignable_to(&Type::Undefined, &void_union));
        assert!(is_assignable_to(&Type::Void, &void_union));
        assert!(is_assignable_to(&void_union, &Type::Void));
        assert!(!is_assignable_to(&void_union, &Type::Undefined));
    }

    #[test]
    fn function_type_assignable_same_signature() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number), is_variadic: false });

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_different_arity() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String, Type::Number],
            return_type: Box::new(Type::Number), is_variadic: false });

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_parameter_mismatch() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number), is_variadic: false });

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_not_assignable_return_mismatch() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::String), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Number), is_variadic: false });

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_return_covariance_basic() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::StringLiteral("ok".to_string())), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::String), is_variadic: false });

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_literal_parameter_conservative() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::StringLiteral("ok".to_string())],
            return_type: Box::new(Type::Void), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Void), is_variadic: false });

        assert!(!is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_union_parameter_compatible_when_same() {
        let source = Type::Function(FunctionType {
            parameters: vec![union_type(vec![Type::String, Type::Number])],
            return_type: Box::new(Type::Void), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![union_type(vec![Type::String, Type::Number])],
            return_type: Box::new(Type::Void), is_variadic: false });

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_void_return_assignable_to_void_return() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Void), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Void), is_variadic: false });

        assert!(is_assignable_to(&source, &target));
    }

    #[test]
    fn function_type_void_return_not_assignable_to_string_return() {
        let source = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Void), is_variadic: false });
        let target = Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::String), is_variadic: false });

        assert!(!is_assignable_to(&source, &target));
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
    fn array_type_name_literal() {
        assert_eq!(
            Type::Array(Box::new(Type::StringLiteral("ok".to_string()))).name(),
            r#""ok"[]"#
        );
    }

    #[test]
    fn array_type_name_union() {
        assert_eq!(
            Type::Array(Box::new(union_type(vec![Type::String, Type::Number]))).name(),
            "(string | number)[]"
        );
    }

    #[test]
    fn array_type_name_function() {
        assert_eq!(
            Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(Type::String), is_variadic: false })))
            .name(),
            "(() => string)[]"
        );
    }

    #[test]
    fn array_assignable_same_element() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_assignable_literal_element_to_base_element() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::StringLiteral("ok".to_string()))),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_not_assignable_base_element_to_literal_element() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::StringLiteral("ok".to_string())))
        ));
    }

    #[test]
    fn array_not_assignable_different_element() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Number)),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_any_element_assignability() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::Any)),
            &Type::Array(Box::new(Type::String))
        ));
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Array(Box::new(Type::Any))
        ));
    }

    #[test]
    fn array_not_assignable_to_primitive() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::String
        ));
    }

    #[test]
    fn primitive_not_assignable_to_array() {
        assert!(!is_assignable_to(
            &Type::String,
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_union_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number]))),
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number])))
        ));
    }

    #[test]
    fn array_union_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Boolean)),
            &Type::Array(Box::new(union_type(vec![Type::String, Type::Number])))
        ));
    }

    #[test]
    fn array_function_element_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(Type::Void), is_variadic: false }))),
            &Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(Type::Void), is_variadic: false })))
        ));
    }

    #[test]
    fn array_function_element_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Void), is_variadic: false }))),
            &Type::Array(Box::new(Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(Type::Void), is_variadic: false })))
        ));
    }

    #[test]
    fn tuple_type_name_empty_if_supported() {
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
                union_type(vec![Type::String, Type::Number]),
                Type::Boolean
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
                    return_type: Box::new(Type::Void), is_variadic: false }),
                Type::String,
            ])
            .name(),
            "[() => void, string]"
        );
    }

    #[test]
    fn tuple_type_name_object_element() {
        let mut properties = BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        assert_eq!(
            Type::Tuple(vec![Type::Object(ObjectType { properties }), Type::Number]).name(),
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

    #[test]
    fn tuple_assignable_same_shape() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_assignable_literal_to_base() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![
                Type::StringLiteral("ok".to_string()),
                Type::NumberLiteral(NumberLiteralType {
                    value: "1".to_string(),
                })
            ]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_not_assignable_base_to_literal() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![
                Type::StringLiteral("ok".to_string()),
                Type::NumberLiteral(NumberLiteralType {
                    value: "1".to_string(),
                })
            ])
        ));
    }

    #[test]
    fn tuple_not_assignable_different_length_too_short() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_not_assignable_different_length_too_long() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Tuple(vec![Type::String])
        ));
    }

    #[test]
    fn tuple_not_assignable_element_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::String]),
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
    }

    #[test]
    fn tuple_assignable_to_array_when_elements_compatible() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::StringLiteral("ok".to_string())]),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn tuple_not_assignable_to_array_when_element_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Array(Box::new(Type::String))
        ));
    }

    #[test]
    fn array_not_assignable_to_tuple() {
        assert!(!is_assignable_to(
            &Type::Array(Box::new(Type::String)),
            &Type::Tuple(vec![Type::String, Type::String])
        ));
    }

    #[test]
    fn tuple_any_assignability() {
        assert!(is_assignable_to(
            &Type::Any,
            &Type::Tuple(vec![Type::String, Type::Number])
        ));
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &Type::Any
        ));
    }

    #[test]
    fn tuple_union_assignability_valid() {
        assert!(is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &union_type(vec![
                Type::Tuple(vec![Type::String, Type::Number]),
                Type::String
            ])
        ));
    }

    #[test]
    fn tuple_union_assignability_mismatch() {
        assert!(!is_assignable_to(
            &Type::Tuple(vec![Type::String, Type::Number]),
            &union_type(vec![Type::Tuple(vec![Type::String, Type::String])])
        ));
    }
}
