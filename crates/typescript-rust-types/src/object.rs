use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{FunctionType, Type, union_type};

#[derive(Debug)]
pub struct ObjectType {
    pub properties: Arc<BTreeMap<String, ObjectProperty>>,
    pub string_index_type: Option<Arc<Type>>,
    /// Name of the interface or type alias this object was resolved from, used
    /// only for diagnostic display (tsc shows `'StrictObj'`, not the structural
    /// expansion). Deliberately excluded from equality so assignability and
    /// structural comparisons stay name-agnostic.
    pub alias_name: Option<Arc<str>>,
    /// Construct signature for a class value (static side). When present, the
    /// object is callable with `new`, producing the signature's return type (the
    /// instance type). Static members live in `properties`. Excluded from
    /// equality, like `alias_name`, so structural comparisons stay shape-based.
    pub construct_signature: Option<Arc<FunctionType>>,
}

impl PartialEq for ObjectType {
    fn eq(&self, other: &Self) -> bool {
        self.properties == other.properties && self.string_index_type == other.string_index_type
    }
}

impl Eq for ObjectType {}

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
    pub fn new(
        properties: BTreeMap<String, ObjectProperty>,
        string_index_type: Option<Type>,
    ) -> Self {
        Self {
            properties: Arc::new(properties),
            string_index_type: string_index_type.map(Arc::new),
            alias_name: None,
            construct_signature: None,
        }
    }

    /// Returns a copy tagged with the interface/type-alias name it was resolved
    /// from, for diagnostic display only.
    pub fn with_alias_name(mut self, alias_name: impl Into<Arc<str>>) -> Self {
        self.alias_name = Some(alias_name.into());
        self
    }

    /// Returns a copy carrying a construct signature, marking this object as the
    /// static/value side of a class that can be invoked with `new`.
    pub fn with_construct_signature(mut self, construct_signature: FunctionType) -> Self {
        self.construct_signature = Some(Arc::new(construct_signature));
        self
    }

    pub fn construct_signature(&self) -> Option<&FunctionType> {
        self.construct_signature.as_deref()
    }

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

impl Clone for ObjectType {
    fn clone(&self) -> Self {
        Self {
            properties: self.properties.clone(),
            string_index_type: self.string_index_type.clone(),
            alias_name: self.alias_name.clone(),
            construct_signature: self.construct_signature.clone(),
        }
    }
}
