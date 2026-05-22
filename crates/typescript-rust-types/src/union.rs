use crate::Type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionType {
    pub types: Vec<Type>,
}

pub fn remove_undefined(ty: &Type) -> Type {
    match ty {
        Type::Union(union) => {
            let filtered: Vec<Type> = union
                .types
                .iter()
                .filter(|t| **t != Type::Undefined)
                .cloned()
                .collect();
            union_type(filtered)
        }
        Type::Undefined => Type::Unknown, // Or whatever makes sense, maybe just return it
        _ => ty.clone(),
    }
}

pub fn remove_nullish(ty: &Type) -> Type {
    match ty {
        Type::Union(union) => {
            let filtered: Vec<Type> = union
                .types
                .iter()
                .filter(|t| **t != Type::Undefined && **t != Type::Void)
                .cloned()
                .collect();
            union_type(filtered)
        }
        Type::Undefined | Type::Void => Type::Unknown,
        _ => ty.clone(),
    }
}

pub fn union_type(types: Vec<Type>) -> Type {
    let mut flattened = Vec::new();

    for ty in types {
        match ty {
            Type::Union(union) => flattened.extend(union.types),
            other => flattened.push(other),
        }
    }

    if flattened.iter().any(|ty| matches!(ty, Type::Any)) {
        return Type::Any;
    }

    let mut unique = Vec::new();
    for ty in flattened {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }

    match unique.len() {
        0 => Type::Unknown,
        1 => unique.into_iter().next().unwrap(),
        _ => Type::Union(UnionType { types: unique }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NumberLiteralType, Type};

    #[test]
    fn literal_union_dedupes_exact_duplicates() {
        let ty = union_type(vec![
            Type::StringLiteral("ok".to_string()),
            Type::StringLiteral("ok".to_string()),
        ]);

        assert_eq!(ty, Type::StringLiteral("ok".to_string()));
    }

    #[test]
    fn literal_union_display_stable() {
        let ty = union_type(vec![
            Type::StringLiteral("ok".to_string()),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::BooleanLiteral(true),
        ]);

        assert_eq!(ty.name(), r#""ok" | 1 | true"#);
    }

    #[test]
    fn literal_union_dedupes_string_literals() {
        let ty = union_type(vec![
            Type::StringLiteral("idle".to_string()),
            Type::StringLiteral("idle".to_string()),
        ]);

        assert_eq!(ty, Type::StringLiteral("idle".to_string()));
    }

    #[test]
    fn literal_union_dedupes_number_literals() {
        let ty = union_type(vec![
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
        ]);

        assert_eq!(
            ty,
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            })
        );
    }

    #[test]
    fn literal_union_dedupes_boolean_literals() {
        let ty = union_type(vec![Type::BooleanLiteral(true), Type::BooleanLiteral(true)]);

        assert_eq!(ty, Type::BooleanLiteral(true));
    }

    #[test]
    fn literal_union_preserves_first_seen_order() {
        let ty = union_type(vec![
            Type::StringLiteral("idle".to_string()),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::BooleanLiteral(true),
            Type::StringLiteral("idle".to_string()),
        ]);

        assert_eq!(ty.name(), r#""idle" | 1 | true"#);
    }

    #[test]
    fn literal_union_does_not_collapse_to_primitive() {
        let ty = union_type(vec![Type::StringLiteral("ok".to_string()), Type::String]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), r#""ok" | string"#);
    }

    #[test]
    fn literal_union_does_not_collapse_string_literal_with_string() {
        let ty = union_type(vec![Type::StringLiteral("idle".to_string()), Type::String]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), r#""idle" | string"#);
    }

    #[test]
    fn literal_union_does_not_collapse_number_literal_with_number() {
        let ty = union_type(vec![
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::Number,
        ]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), "1 | number");
    }

    #[test]
    fn literal_union_does_not_collapse_boolean_literal_with_boolean() {
        let ty = union_type(vec![Type::BooleanLiteral(true), Type::Boolean]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), "true | boolean");
    }

    #[test]
    fn literal_union_with_any_collapses_to_any() {
        let ty = union_type(vec![Type::StringLiteral("ok".to_string()), Type::Any]);

        assert_eq!(ty, Type::Any);
    }

    #[test]
    fn literal_union_flattens_nested_literal_unions() {
        let ty = union_type(vec![
            Type::Union(UnionType {
                types: vec![
                    Type::StringLiteral("idle".to_string()),
                    Type::NumberLiteral(NumberLiteralType {
                        value: "1".to_string(),
                    }),
                ],
            }),
            Type::Union(UnionType {
                types: vec![
                    Type::BooleanLiteral(true),
                    Type::StringLiteral("idle".to_string()),
                ],
            }),
        ]);

        assert_eq!(ty.name(), r#""idle" | 1 | true"#);
    }
}
