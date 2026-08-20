use std::sync::Arc;

use indexmap::IndexMap;

use crate::store::{canonical_property_map_store_enabled, current_program_type_store};
use crate::{FunctionType, PropertyMapId, Type, union_type};

/// Property map preserving declaration order, which tsc relies on when rendering
/// object types in diagnostics (`{ disabled?: boolean; children?: unknown }`).
/// Equality is order-independent, matching the previous `BTreeMap` semantics, so
/// structural comparisons and type caching are unaffected.
// Insertion-ordered like any IndexMap (iteration order is hasher-independent),
// with the fast workspace hasher for the per-lookup cost.
//
// Keys are `Arc<str>`, not `String`: a derived interface inherits its bases'
// members by cloning the base map's keys (interface member merge), and the base
// maps are `Arc`-shared and reused across every derived type, so an `Arc<str>`
// key makes each inherited-member clone a refcount bump and lets all derived
// types share one allocation per base member name. Equality is by str content
// (order-independent, as before) and `Arc<str>: Hash` matches `String::hash`
// byte-for-byte, so structural comparison, the dedup fingerprint, and the
// canonical property-map store are unchanged.
pub type PropertyMap = IndexMap<Arc<str>, ObjectProperty, crate::fx::FxBuildHasher>;

#[derive(Debug)]
pub struct ObjectType {
    pub properties: Arc<PropertyMap>,
    pub property_map_id: Option<PropertyMapId>,
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
    /// instance type). Static members live in `properties`. Part of equality —
    /// see `signatures_equal`.
    pub construct_signature: Option<Arc<FunctionType>>,
    /// Call signature for a callable object (a `declare var Number: NumberConstructor`
    /// style value whose interface has a `(value?: any): number` signature). When
    /// present, the object is callable without `new`, producing the signature's
    /// return type. Part of equality, like `construct_signature`.
    pub call_signature: Option<Arc<FunctionType>>,
    /// Set when this object is the merged surface of an intersection (`A & B`).
    /// Used only to pick the diagnostic tsc reports for a missing required
    /// property (intersections surface the outer assignability code, e.g.
    /// `TS2322`/`TS2345`, rather than the standalone `TS2741`). Excluded from
    /// equality so intersection-merged objects compare structurally.
    pub is_intersection: bool,
    /// Set when `string_index_type` was injected by the checker to keep an
    /// intersection surface open (an operand it could not enumerate), not
    /// declared by the source. The index still widens property *reads*, but
    /// `noPropertyAccessFromIndexSignature` must not fire on it: the property
    /// really lives on the operand surge dropped, not on an index signature the
    /// author wrote. Excluded from equality, like `is_intersection`, so no cache
    /// key, dedup fingerprint, or canonical-store identity changes.
    pub synthetic_open_index: bool,
}

impl PartialEq for ObjectType {
    fn eq(&self, other: &Self) -> bool {
        // Object types resolved from the same memoized reference share a property
        // map `Arc`; comparing their pointers short-circuits the O(size) structural
        // compare, which dominates checking projects with large library object
        // graphs (DOM `Request`/`Response`, …) reached through nominal references.
        let properties_equal =
            Arc::ptr_eq(&self.properties, &other.properties) || self.properties == other.properties;
        properties_equal
            && self.string_index_type == other.string_index_type
            && signatures_equal(&self.construct_signature, &other.construct_signature)
            && signatures_equal(&self.call_signature, &other.call_signature)
    }
}

/// Call/construct signatures participate in equality: two class *static* sides
/// (`typeof AppendCommand` vs `typeof PExpireAtCommand`) usually carry identical
/// (often empty) static property maps and differ only in their constructor, so
/// leaving signatures out made them compare equal — and any cache keyed on
/// resolved type arguments (the generic-instantiation interner) then served one
/// class's expansion for another's.
fn signatures_equal(left: &Option<Arc<FunctionType>>, right: &Option<Arc<FunctionType>>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Arc::ptr_eq(left, right) || left == right,
        _ => false,
    }
}

impl Eq for ObjectType {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectProperty {
    pub ty: Type,
    pub optional: bool,
    /// Declared with method syntax (`m(): T`) rather than as a property holding
    /// a function type (`m: () => T`). tsc checks a method's parameters
    /// bivariantly even under `strictFunctionTypes`, so the distinction is
    /// load-bearing for assignability.
    pub method: bool,
}

impl ObjectProperty {
    pub fn required(ty: Type) -> Self {
        Self {
            ty,
            optional: false,
            method: false,
        }
    }

    pub fn optional(ty: Type) -> Self {
        Self {
            ty,
            optional: true,
            method: false,
        }
    }

    pub fn with_method(mut self, method: bool) -> Self {
        self.method = method;
        self
    }

    pub fn is_optional(&self) -> bool {
        self.optional
    }

    pub fn is_required(&self) -> bool {
        !self.optional
    }

    pub fn is_method(&self) -> bool {
        self.method
    }
}

impl ObjectType {
    pub fn new(properties: PropertyMap, string_index_type: Option<Type>) -> Self {
        let (properties, property_map_id) = if canonical_property_map_store_enabled()
            && let Some(store) = current_program_type_store()
        {
            match store.intern_property_map(properties) {
                Ok((map, id)) => (map, Some(id)),
                Err(properties) => (Arc::new(properties), None),
            }
        } else {
            (Arc::new(properties), None)
        };
        Self {
            properties,
            property_map_id,
            string_index_type: string_index_type.map(Arc::new),
            alias_name: None,
            alias_id: None,
            construct_signature: None,
            call_signature: None,
            is_intersection: false,
            synthetic_open_index: false,
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

    /// Marks this object's string index signature as checker-injected openness
    /// rather than a declared `[key: string]: T`.
    pub fn with_open_index_marker(mut self) -> Self {
        self.synthetic_open_index = true;
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

        if let Some(indexed) = self.string_index_type.as_deref() {
            return Some(indexed.clone());
        }

        // Every object type's apparent type includes the global `Object`
        // interface members, so `error.toString()` / `value.hasOwnProperty(k)`
        // resolve even when the declared interface (e.g. lib's `Error`) never
        // restates them. Without this fallback the access was a TS2339 false
        // positive. Only consulted after declared members and the index
        // signature, mirroring tsc's member-precedence order.
        object_prototype_member_type(name)
    }

    pub fn contains_property(&self, name: &str) -> bool {
        self.properties.contains_key(name) || self.string_index_type.is_some()
    }

    pub fn allows_string_index_access(&self) -> bool {
        self.string_index_type.is_some()
    }

    pub fn required_properties(&self) -> impl Iterator<Item = (&Arc<str>, &ObjectProperty)> + '_ {
        self.properties.iter().filter(|entry| entry.1.is_required())
    }

    pub fn optional_properties(&self) -> impl Iterator<Item = (&Arc<str>, &ObjectProperty)> + '_ {
        self.properties.iter().filter(|entry| entry.1.is_optional())
    }
}

/// The members of the global `Object` interface (lib.es5), shared by the apparent
/// type of every non-nullish value. Parameter types are approximated as `any`
/// (the real signatures take `PropertyKey`/`Object`) since only arity and the
/// return type matter for the diagnostics surge emits.
fn object_prototype_member_type(name: &str) -> Option<Type> {
    let member = match name {
        "toString" | "toLocaleString" => FunctionType::new(vec![], Type::String, false, 0),
        "valueOf" => FunctionType::new(vec![], Type::Any, false, 0),
        "hasOwnProperty" | "isPrototypeOf" | "propertyIsEnumerable" => {
            FunctionType::new(vec![Type::Any], Type::Boolean, false, 1)
        }
        "constructor" => return Some(Type::Any),
        _ => return None,
    };
    Some(Type::Function(member))
}

impl Clone for ObjectType {
    fn clone(&self) -> Self {
        Self {
            properties: self.properties.clone(),
            property_map_id: self.property_map_id,
            string_index_type: self.string_index_type.clone(),
            alias_name: self.alias_name.clone(),
            alias_id: self.alias_id.clone(),
            construct_signature: self.construct_signature.clone(),
            call_signature: self.call_signature.clone(),
            is_intersection: self.is_intersection,
            synthetic_open_index: self.synthetic_open_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FunctionType;

    fn constructor(parameter: Type) -> FunctionType {
        FunctionType::new(vec![parameter], Type::Number, false, 1)
    }

    #[test]
    fn static_sides_differing_only_in_their_constructor_are_not_equal() {
        // Two class static sides carry the same (here empty) static property map
        // and differ only in the constructor, so leaving signatures out of
        // equality made every `typeof SomeClass` interchangeable — and caches
        // keyed on resolved type arguments served one class's expansion for
        // another's.
        let append = ObjectType::new(PropertyMap::default(), None)
            .with_construct_signature(constructor(Type::String));
        let expire = ObjectType::new(PropertyMap::default(), None)
            .with_construct_signature(constructor(Type::Number));
        assert_ne!(append, expire);

        let same = ObjectType::new(PropertyMap::default(), None)
            .with_construct_signature(constructor(Type::String));
        assert_eq!(append, same);
    }

    #[test]
    fn the_synthetic_open_index_marker_stays_out_of_equality() {
        // Like `is_intersection`: the marker only steers diagnostic selection, so
        // it must not split cache keys, dedup fingerprints, or store identity.
        let declared = ObjectType::new(PropertyMap::default(), Some(Type::Any));
        let synthetic =
            ObjectType::new(PropertyMap::default(), Some(Type::Any)).with_open_index_marker();
        assert_eq!(declared, synthetic);
        assert!(!declared.synthetic_open_index);
        assert!(synthetic.synthetic_open_index);
        assert!(synthetic.clone().synthetic_open_index);
    }

    #[test]
    fn a_synthetic_open_index_is_not_an_index_signature_source() {
        let declared = Type::Object(ObjectType::new(PropertyMap::default(), Some(Type::Any)));
        let synthetic = Type::Object(
            ObjectType::new(PropertyMap::default(), Some(Type::Any)).with_open_index_marker(),
        );
        assert!(declared.property_only_from_string_index("path"));
        assert!(!synthetic.property_only_from_string_index("path"));
    }

    #[test]
    fn a_callable_object_is_not_equal_to_a_plain_one() {
        let plain = ObjectType::new(PropertyMap::default(), None);
        let callable = ObjectType::new(PropertyMap::default(), None)
            .with_call_signature(constructor(Type::String));
        assert_ne!(plain, callable);
    }
}
