use std::collections::BTreeMap;

use crate::{Type, union_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectType {
    pub properties: BTreeMap<String, ObjectProperty>,
    pub string_index_type: Option<Box<Type>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectProperty {
    pub ty: Type,
    pub optional: bool,
}

impl ObjectProperty {
    pub fn required(ty: Type) -> Self {
        Self {
            ty,
            optional: false,
        }
    }

    pub fn optional(ty: Type) -> Self {
        Self { ty, optional: true }
    }

    pub fn is_optional(&self) -> bool {
        self.optional
    }

    pub fn is_required(&self) -> bool {
        !self.optional
    }
}

impl ObjectType {
    pub fn get_property(&self, name: &str) -> Option<&ObjectProperty> {
        self.properties.get(name)
    }

    pub fn get_property_type(&self, name: &str) -> Option<&Type> {
        self.properties.get(name).map(|property| &property.ty)
    }

    pub fn get_property_access_type(&self, name: &str) -> Option<Type> {
        if let Some(property) = self.properties.get(name) {
            if property.is_optional() {
                return Some(union_type(vec![property.ty.clone(), Type::Undefined]));
            }

            return Some(property.ty.clone());
        }

        self.string_index_type.as_deref().cloned()
    }

    pub fn contains_property(&self, name: &str) -> bool {
        self.properties.contains_key(name) || self.string_index_type.is_some()
    }

    pub fn allows_string_index_access(&self) -> bool {
        self.string_index_type.is_some()
    }

    pub fn required_properties(&self) -> impl Iterator<Item = (&String, &ObjectProperty)> + '_ {
        self.properties.iter().filter(|entry| entry.1.is_required())
    }

    pub fn optional_properties(&self) -> impl Iterator<Item = (&String, &ObjectProperty)> + '_ {
        self.properties.iter().filter(|entry| entry.1.is_optional())
    }
}
