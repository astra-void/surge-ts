//! Core TypeScript-like type representation and assignability helpers.

mod assignability;
mod function;
mod object;
mod ty;
mod union;

pub use assignability::*;
pub use function::*;
pub use object::*;
pub use ty::*;
pub use union::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn crate_root_reexports_still_work() {
        let mut properties = BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        let ty = Type::Object(ObjectType { properties });

        assert_eq!(ty.name(), "{ name: string; }");
        assert!(is_assignable_to(&Type::String, &Type::Any));
    }

    #[test]
    fn optional_property_access_widens_to_undefined() {
        let mut properties = BTreeMap::new();
        properties.insert("name".to_string(), ObjectProperty::optional(Type::String));

        let ty = ObjectType { properties };

        assert_eq!(
            ty.get_property_access_type("name"),
            Some(union_type(vec![Type::String, Type::Undefined]))
        );
    }
}
