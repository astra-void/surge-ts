//! Core TypeScript-like type representation and assignability helpers.

mod assignability;
mod clone_reason;
mod function;
mod object;
mod reference;
mod ty;
mod union;

pub use assignability::*;
pub use clone_reason::{TypeCopyReason, with_type_copy_reason};
pub use function::*;
pub use object::*;
pub use reference::*;
pub use ty::*;
pub use union::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyMap;

    #[test]
    fn crate_root_reexports_still_work() {
        let mut properties = PropertyMap::new();
        properties.insert("name".to_string(), ObjectProperty::required(Type::String));

        let ty = Type::Object(ObjectType::new(properties, None));

        assert_eq!(ty.name(), "{ name: string; }");
        assert!(is_assignable_to(&Type::String, &Type::Any));
    }

    #[test]
    fn optional_property_access_widens_to_undefined() {
        let mut properties = PropertyMap::new();
        properties.insert("name".to_string(), ObjectProperty::optional(Type::String));

        let ty = ObjectType::new(properties, None);

        assert_eq!(
            ty.get_property_access_type("name"),
            Some(union_type(vec![Type::String, Type::Undefined]))
        );
    }
}
