use std::sync::Arc;

use indexmap::IndexMap;

use crate::{FunctionType, Type, union_type};

/// Property map preserving declaration order, which tsc relies on when rendering
/// object types in diagnostics (`{ disabled?: boolean; children?: unknown }`).
/// Equality is order-independent, matching the previous `BTreeMap` semantics, so
/// structural comparisons and type caching are unaffected.
pub type PropertyMap = IndexMap<String, ObjectProperty>;

#[derive(Debug)]
pub struct ObjectType {
    pub properties: Arc<PropertyMap>,
    pub string_index_type: Option<Arc<Type>>,
    /// Name of the interface or type alias this object was resolved from, used
    /// only for diagnostic display (tsc shows `'StrictObj'`, not the structural
    /// expansion). Deliberately excluded from equality so assignability and
    /// structural comparisons stay name-agnostic.
    pub alias_name: Option<Arc<str>>,
    /// Nominal identity of the non-generic interface/type-alias declaration this
    /// object was resolved from (qualified `file::name`). Two objects resolved
    /// from the same declaration share it; assignability treats them as the same
    /// named type, matching tsc's nominal handling and avoiding spurious failures
    /// when a deeply cyclic library type (e.g. `Buffer`) expands to structurally
    /// different shapes at different sites. Excluded from equality, like
    /// `alias_name`, so structural comparisons stay shape-based.
    pub alias_id: Option<Arc<str>>,
    /// Construct signature for a class value (static side). When present, the
    /// object is callable with `new`, producing the signature's return type (the
    /// instance type). Static members live in `properties`. Excluded from
    /// equality, like `alias_name`, so structural comparisons stay shape-based.
    pub construct_signature: Option<Arc<FunctionType>>,
    /// Call signature for a callable object (a `declare var Number: NumberConstructor`
    /// style value whose interface has a `(value?: any): number` signature). When
    /// present, the object is callable without `new`, producing the signature's
    /// return type. Excluded from equality, like `construct_signature`.
    pub call_signature: Option<Arc<FunctionType>>,
    /// Set when this object is the merged surface of an intersection (`A & B`).
    /// Used only to pick the diagnostic tsc reports for a missing required
    /// property (intersections surface the outer assignability code, e.g.
    /// `TS2322`/`TS2345`, rather than the standalone `TS2741`). Excluded from
    /// equality so intersection-merged objects compare structurally.
    pub is_intersection: bool,
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
    pub fn new(properties: PropertyMap, string_index_type: Option<Type>) -> Self {
        Self {
            properties: Arc::new(properties),
            string_index_type: string_index_type.map(Arc::new),
            alias_name: None,
            alias_id: None,
            construct_signature: None,
            call_signature: None,
            is_intersection: false,
        }
    }

    /// Returns a copy tagged with the interface/type-alias name it was resolved
    /// from, for diagnostic display only.
    pub fn with_alias_name(mut self, alias_name: impl Into<Arc<str>>) -> Self {
        self.alias_name = Some(alias_name.into());
        self
    }

    /// Returns a copy tagged with the nominal identity of its source declaration.
    pub fn with_alias_id(mut self, alias_id: impl Into<Arc<str>>) -> Self {
        self.alias_id = Some(alias_id.into());
        self
    }

    /// Marks this object as the merged surface of an intersection type.
    pub fn with_intersection_marker(mut self) -> Self {
        self.is_intersection = true;
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

    /// Returns a copy carrying a call signature, marking this object as callable
    /// without `new` (e.g. `Number(value)`).
    pub fn with_call_signature(mut self, call_signature: FunctionType) -> Self {
        self.call_signature = Some(Arc::new(call_signature));
        self
    }

    pub fn call_signature(&self) -> Option<&FunctionType> {
        self.call_signature.as_deref()
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
            alias_id: self.alias_id.clone(),
            construct_signature: self.construct_signature.clone(),
            call_signature: self.call_signature.clone(),
            is_intersection: self.is_intersection,
        }
    }
}
