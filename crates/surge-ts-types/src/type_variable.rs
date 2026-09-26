//! Type variables of the generic body currently being checked.
//!
//! tsc gives every type parameter an identity and a constraint, and relates a
//! type variable through that constraint (`structuredTypeRelatedToWorker` in
//! relater.go). surge's [`Type::TypeParameter`] placeholder instead stands in
//! for anything it could not instantiate, so it relates like the degradation
//! sentinel. A body's *own* parameters are the exception: while that body is
//! checked they are real variables, bound here with their constraints and
//! marked by a nonzero [`TypeParameterType::owner`]. Once the body is done the
//! entry is gone and any variable that escaped relates like a placeholder
//! again — the constraint lives only as long as the scope that declares it, so
//! no type holds its own constraint alive.
//!
//! The deferred types tsc builds over those variables — `T[K]` and `keyof T`
//! ([`DeferredType`]) — are bound here too, under an owner of their own, for
//! as long as every variable they are built from.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::{Type, TypeParameterType};

struct ActiveVariable {
    owner: u32,
    name: Arc<str>,
    declaration: (Arc<str>, u32),
    constraint: Option<Type>,
    deferred: Option<Deferred>,
}

struct Deferred {
    kind: DeferredType,
    /// The declared variables the type is built from, by owner and name.
    bases: Vec<(u32, Arc<str>)>,
}

/// A generic type tsc leaves unresolved while one of its operands is a type
/// variable: `T[K]` (`shouldDeferIndexedAccessType`) and `keyof T`
/// (`shouldDeferIndexType`). It is a type variable of its own, related through
/// the rules relater.go gives `IndexedAccess` and `Index` types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeferredType {
    IndexedAccess { object: Type, index: Type },
    Keyof(Type),
}

/// What a target-side rule of a deferred type admits: the types a source has
/// to relate to, nothing (`Absent`), or a question surge cannot answer.
pub enum TargetConstraint {
    Types(Vec<Type>),
    Absent,
    Unmodelled,
}

thread_local! {
    static ACTIVE_VARIABLES: RefCell<Vec<ActiveVariable>> = const { RefCell::new(Vec::new()) };
    static NEXT_OWNER: Cell<u32> = const { Cell::new(1) };
    static CONSTRAINT_DEPTH: Cell<u32> = const { Cell::new(0) };
}

fn next_owner() -> u32 {
    NEXT_OWNER.with(|next| {
        let owner = next.get();
        next.set(owner.wrapping_add(1).max(1));
        owner
    })
}

/// The variables one generic body binds. Dropping it unbinds them.
pub struct TypeVariableScope {
    owner: u32,
}

impl TypeVariableScope {
    /// Binds `names` (each with its declaration identity: file and name offset)
    /// as variables of a fresh owner. Constraints are set afterwards with
    /// [`TypeVariableScope::set_constraint`], because resolving one may name the
    /// variables themselves (`U extends T`, `T extends Foo<T>`).
    pub fn enter(parameters: impl IntoIterator<Item = (Arc<str>, (Arc<str>, u32))>) -> Self {
        let owner = next_owner();
        ACTIVE_VARIABLES.with(|active| {
            active.borrow_mut().extend(parameters.into_iter().map(|(name, declaration)| {
                ActiveVariable {
                    owner,
                    name,
                    declaration,
                    constraint: None,
                    deferred: None,
                }
            }))
        });
        Self { owner }
    }

    pub fn set_constraint(&self, name: &str, constraint: Type) {
        ACTIVE_VARIABLES.with(|active| {
            if let Some(variable) = active
                .borrow_mut()
                .iter_mut()
                .find(|variable| variable.owner == self.owner && *variable.name == *name)
            {
                variable.constraint = Some(constraint);
            }
        });
    }

    /// Unbinds `name`: a constraint surge cannot model says nothing about what
    /// the variable admits, so it relates like a placeholder again, and a
    /// variable already built for it turns permissive with it.
    pub fn forget(&self, name: &str) {
        ACTIVE_VARIABLES.with(|active| {
            active.borrow_mut().retain(|variable| {
                !(variable.owner == self.owner && *variable.name == *name)
                    && !builds_on(variable, |owner, base| owner == self.owner && *base == *name)
            })
        });
    }

    pub fn variable(&self, name: &str) -> Type {
        Type::TypeParameter(TypeParameterType {
            name: Arc::from(name),
            owner: self.owner,
        })
    }
}

impl Drop for TypeVariableScope {
    fn drop(&mut self) {
        ACTIVE_VARIABLES.with(|active| {
            active
                .borrow_mut()
                .retain(|variable| variable.owner != self.owner && !builds_on(variable, |owner, _| owner == self.owner))
        });
    }
}

fn builds_on(variable: &ActiveVariable, base: impl Fn(u32, &str) -> bool) -> bool {
    variable
        .deferred
        .as_ref()
        .is_some_and(|deferred| deferred.bases.iter().any(|(owner, name)| base(*owner, name)))
}

/// The active variable a type-parameter *declaration* is bound to, if its
/// body is being checked.
pub fn variable_for_declaration(file: &str, name_offset: u32) -> Option<Type> {
    ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .rev()
            .find(|variable| {
                variable.deferred.is_none() && *variable.declaration.0 == *file && variable.declaration.1 == name_offset
            })
            .map(|variable| {
                Type::TypeParameter(TypeParameterType {
                    name: variable.name.clone(),
                    owner: variable.owner,
                })
            })
    })
}

/// `Some(constraint)` when `parameter` is a variable whose body is being
/// checked (`None` inside for an unconstrained one); `None` for a placeholder.
/// A deferred type answers its base constraint, or `None` when surge cannot
/// compute it.
pub fn active_constraint(parameter: &TypeParameterType) -> Option<Option<Type>> {
    if parameter.owner == 0 {
        return None;
    }
    let (constraint, deferred) = ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .find(|variable| variable.owner == parameter.owner && *variable.name == *parameter.name)
            .map(|variable| {
                (
                    variable.constraint.clone(),
                    variable.deferred.as_ref().map(|deferred| deferred.kind.clone()),
                )
            })
    })?;
    match deferred {
        None => Some(constraint),
        Some(kind) => deferred_constraint(&kind),
    }
}

/// What `parameter` is built from when it is a deferred type.
pub fn deferred_type(parameter: &TypeParameterType) -> Option<DeferredType> {
    if parameter.owner == 0 {
        return None;
    }
    ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .find(|variable| variable.owner == parameter.owner && *variable.name == *parameter.name)
            .and_then(|variable| variable.deferred.as_ref().map(|deferred| deferred.kind.clone()))
    })
}

/// tsc's deferred `object[index]` over a type variable `object`, or `None`
/// when an operand is not something the deferred type can be built from (a
/// placeholder, the sentinel, a key that is no key type).
pub fn indexed_access_variable(object: &Type, index: &Type) -> Option<Type> {
    let mut bases = variable_bases(object)?;
    bases.extend(index_bases(index)?);
    let object_name = object.name();
    let object_name = if object_name.starts_with("keyof ") {
        format!("({object_name})")
    } else {
        object_name
    };
    deferred_variable(
        DeferredType::IndexedAccess {
            object: object.clone(),
            index: index.clone(),
        },
        format!("{object_name}[{}]", index.name()),
        bases,
    )
}

/// tsc's deferred `keyof operand` over a type variable `operand`.
pub fn keyof_variable(operand: &Type) -> Option<Type> {
    let bases = variable_bases(operand)?;
    deferred_variable(DeferredType::Keyof(operand.clone()), format!("keyof {}", operand.name()), bases)
}

fn deferred_variable(kind: DeferredType, name: String, bases: Vec<(u32, Arc<str>)>) -> Option<Type> {
    if bases.is_empty() {
        return None;
    }
    ACTIVE_VARIABLES.with(|active| {
        let mut active = active.borrow_mut();
        if let Some(existing) = active
            .iter()
            .find(|variable| variable.deferred.as_ref().is_some_and(|deferred| deferred.kind == kind))
        {
            return Some(Type::TypeParameter(TypeParameterType {
                name: existing.name.clone(),
                owner: existing.owner,
            }));
        }
        let owner = next_owner();
        let name: Arc<str> = Arc::from(name);
        active.push(ActiveVariable {
            owner,
            name: name.clone(),
            declaration: (Arc::from(""), 0),
            constraint: None,
            deferred: Some(Deferred { kind, bases }),
        });
        Some(Type::TypeParameter(TypeParameterType { name, owner }))
    })
}

/// The declared variables `ty` is built from, when it is an active variable.
fn variable_bases(ty: &Type) -> Option<Vec<(u32, Arc<str>)>> {
    let Type::TypeParameter(parameter) = ty else {
        return None;
    };
    if parameter.owner == 0 {
        return None;
    }
    ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .find(|variable| variable.owner == parameter.owner && *variable.name == *parameter.name)
            .map(|variable| match &variable.deferred {
                Some(deferred) => deferred.bases.clone(),
                None => vec![(variable.owner, variable.name.clone())],
            })
    })
}

fn index_bases(index: &Type) -> Option<Vec<(u32, Arc<str>)>> {
    match index {
        Type::TypeParameter(_) => variable_bases(index),
        Type::String | Type::Number | Type::Symbol | Type::StringLiteral(_) | Type::NumberLiteral(_) => {
            Some(Vec::new())
        }
        Type::Union(union) => {
            let mut bases = Vec::new();
            for member in union.types() {
                bases.extend(index_bases(member)?);
            }
            Some(bases)
        }
        _ => None,
    }
}

/// Bounds the constraint walks below: a chain of constraints naming one
/// another is circular (tsc's `circularConstraintType`).
struct ConstraintDepth;

impl ConstraintDepth {
    fn enter() -> Option<Self> {
        CONSTRAINT_DEPTH.with(|depth| {
            let current = depth.get();
            (current < 32).then(|| {
                depth.set(current + 1);
                ConstraintDepth
            })
        })
    }
}

impl Drop for ConstraintDepth {
    fn drop(&mut self) {
        CONSTRAINT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// `keyofConstraintType`: what every `keyof T` is constrained to.
fn key_constraint() -> Type {
    crate::union_type(vec![Type::String, Type::Number, Type::Symbol])
}

fn deferred_constraint(kind: &DeferredType) -> Option<Option<Type>> {
    match kind {
        // `computeBaseConstraint` for an `Index` type.
        DeferredType::Keyof(_) => Some(Some(key_constraint())),
        // `computeBaseConstraint` for an `IndexedAccess` type: the property
        // the key's base constraint selects out of the object's.
        DeferredType::IndexedAccess { object, index } => {
            let _depth = ConstraintDepth::enter()?;
            let (Some(object), Some(index)) = (base_constraint(object)?, base_constraint(index)?) else {
                return Some(None);
            };
            if is_generic(&object) || is_generic(&index) {
                return None;
            }
            Some(indexed_access_lookup(&object, &index, false)?.map(crate::union_type))
        }
    }
}

/// tsc's `getBaseConstraintOfType`: `Some(None)` for a variable without a
/// constraint, `None` for one surge cannot resolve.
fn base_constraint(ty: &Type) -> Option<Option<Type>> {
    let _depth = ConstraintDepth::enter()?;
    match ty {
        Type::TypeParameter(parameter) => match active_constraint(parameter)? {
            None => Some(None),
            Some(constraint) if constraint == *ty => Some(None),
            Some(constraint) => base_constraint(&constraint),
        },
        Type::Union(union) => {
            let mut members = Vec::with_capacity(union.types().len());
            for member in union.types() {
                match base_constraint(member)? {
                    Some(member) => members.push(member),
                    None => return Some(None),
                }
            }
            Some(Some(crate::union_type(members)))
        }
        Type::GenuineUnknown => Some(Some(Type::GenuineUnknown)),
        other if other.is_unmodelled() => None,
        other => Some(Some(other.clone())),
    }
}

/// tsc's `getBaseConstraintOrType`.
fn base_constraint_or_self(ty: &Type) -> Option<Type> {
    Some(base_constraint(ty)?.unwrap_or_else(|| ty.clone()))
}

/// `isGenericObjectType` / `isGenericIndexType` for what a base constraint
/// can leave behind. An intersection holding a variable is merged, not
/// generic here: the lookup reads it as unmodelled.
fn is_generic(ty: &Type) -> bool {
    match ty {
        Type::TypeParameter(_) => true,
        Type::Union(union) => union.types().iter().any(is_generic),
        _ => false,
    }
}

enum Lookup {
    Found(Type),
    Missing,
    Unmodelled,
}

/// tsc's `getIndexedAccessTypeOrUndefined` with no access node, over a
/// non-generic object and key: the type each key constituent selects,
/// `Some(None)` when one selects nothing, `None` when surge cannot tell.
fn indexed_access_lookup(object: &Type, index: &Type, no_index_signatures: bool) -> Option<Option<Vec<Type>>> {
    let keys = match index {
        Type::Union(union) => union.types(),
        other => std::slice::from_ref(other),
    };
    let mut types = Vec::with_capacity(keys.len());
    let mut unmodelled = false;
    for key in keys {
        match property_type_for_index(object, key, no_index_signatures) {
            Lookup::Found(ty) => types.push(ty),
            Lookup::Missing => return Some(None),
            Lookup::Unmodelled => unmodelled = true,
        }
    }
    (!unmodelled).then_some(Some(types))
}

/// tsc's `getPropertyTypeForIndexType`: a literal key names a property, and
/// any key falls back to the index signature that applies to it — a symbol
/// key to the string one. `NoIndexSignatures` (a type variable's constraint
/// written through) admits only a number index.
fn property_type_for_index(object: &Type, key: &Type, no_index_signatures: bool) -> Lookup {
    let object = object.peeled();
    if matches!(object, Type::Any | Type::Never) {
        return Lookup::Found(object);
    }
    let (name, numeric) = match key {
        Type::StringLiteral(value) => (Some(value.as_str()), crate::is_numeric_key(value)),
        Type::NumberLiteral(literal) => (Some(literal.value.as_str()), true),
        Type::String | Type::Symbol => (None, false),
        Type::Number => (None, true),
        _ => return Lookup::Unmodelled,
    };
    match &object {
        Type::Object(object) => {
            if object.synthetic_open_index || object.intersection_operands.is_some() {
                return Lookup::Unmodelled;
            }
            if let Some(name) = name {
                if let Some(property) = object.properties.get(name) {
                    if property.restriction.is_some() || property.index_slot {
                        return Lookup::Unmodelled;
                    }
                    return Lookup::Found(if property.optional {
                        crate::union_type(vec![property.ty.clone(), Type::Undefined])
                    } else {
                        property.ty.clone()
                    });
                }
                if crate::object_prototype_member_type(name).is_some() {
                    return Lookup::Unmodelled;
                }
            }
            if numeric && let Some(value) = object.number_index_type.as_deref() {
                return Lookup::Found(value.clone());
            }
            match object.string_index_type.as_deref() {
                Some(_) if no_index_signatures => Lookup::Missing,
                Some(value) => Lookup::Found(value.clone()),
                None => Lookup::Missing,
            }
        }
        Type::Array(element) if numeric => Lookup::Found(element.as_ref().clone()),
        Type::String | Type::StringLiteral(_) if name.is_none() => {
            if numeric {
                Lookup::Found(Type::String)
            } else {
                Lookup::Missing
            }
        }
        Type::Array(_)
        | Type::Number
        | Type::NumberLiteral(_)
        | Type::Boolean
        | Type::BooleanLiteral(_)
        | Type::BigInt
        | Type::Symbol
            if name.is_none() =>
        {
            Lookup::Missing
        }
        Type::Null | Type::Undefined | Type::Void | Type::GenuineUnknown => Lookup::Missing,
        _ => Lookup::Unmodelled,
    }
}

/// The base constraint of `object[index]` for writing, the target side of
/// `structuredTypeRelatedToWorker`'s `IndexedAccess` arm: absent while either
/// base is still generic, and read without the index signatures of a type
/// variable's constraint (`AccessFlags.NoIndexSignatures`).
pub fn indexed_access_write_constraint(object: &Type, index: &Type) -> TargetConstraint {
    let _depth = match ConstraintDepth::enter() {
        Some(depth) => depth,
        None => return TargetConstraint::Unmodelled,
    };
    let (Some(base_object), Some(base_index)) = (base_constraint_or_self(object), base_constraint_or_self(index))
    else {
        return TargetConstraint::Unmodelled;
    };
    if is_generic(&base_object) || is_generic(&base_index) {
        return TargetConstraint::Absent;
    }
    match indexed_access_lookup(&base_object, &base_index, base_object != *object) {
        None => TargetConstraint::Unmodelled,
        Some(None) => TargetConstraint::Absent,
        Some(Some(types)) => TargetConstraint::Types(types),
    }
}

/// The keys a source of `keyof operand` may be: `keyof C` for the constraint
/// `C` of `operand` (the target side of the `Index` arm; relater.go reads the
/// simplified type or constraint of the operand).
pub fn keyof_constraint_keys(operand: &Type) -> TargetConstraint {
    let _depth = match ConstraintDepth::enter() {
        Some(depth) => depth,
        None => return TargetConstraint::Unmodelled,
    };
    let Type::TypeParameter(parameter) = operand else {
        return TargetConstraint::Unmodelled;
    };
    if deferred_type(parameter).is_some() {
        return TargetConstraint::Unmodelled;
    }
    match active_constraint(parameter) {
        None => TargetConstraint::Unmodelled,
        Some(None) => TargetConstraint::Absent,
        Some(Some(constraint @ Type::TypeParameter(_))) => keyof_constraint_keys(&constraint),
        Some(Some(constraint)) => match keys_of(&constraint) {
            Some(keys) => TargetConstraint::Types(vec![keys]),
            None => TargetConstraint::Unmodelled,
        },
    }
}

/// tsc's `getIndexType` for a non-generic type surge models member by member.
fn keys_of(ty: &Type) -> Option<Type> {
    match ty.peeled() {
        Type::Any | Type::Never => Some(key_constraint()),
        Type::GenuineUnknown | Type::Null | Type::Undefined | Type::Void => Some(Type::Never),
        Type::Object(object) => {
            if object.synthetic_open_index || object.intersection_operands.is_some() {
                return None;
            }
            let mut keys = Vec::new();
            for (name, property) in object.properties.iter() {
                if crate::private_name::is_private_name_key(name) || property.restriction.is_some() {
                    continue;
                }
                // A computed member's key is a symbol, and a numeric name is a
                // number literal key or a string one depending on how it was
                // written; neither is recorded.
                if name.starts_with('[') || crate::is_numeric_key(name) || property.index_slot {
                    return None;
                }
                keys.push(Type::StringLiteral(name.to_string()));
            }
            if object.string_index_type.is_some() {
                keys.extend([Type::String, Type::Number]);
            } else if object.number_index_type.is_some() {
                keys.push(Type::Number);
            }
            Some(if keys.is_empty() { Type::Never } else { crate::union_type(keys) })
        }
        _ => None,
    }
}

impl TypeParameterType {
    pub fn is_active_variable(&self) -> bool {
        self.owner != 0
            && ACTIVE_VARIABLES.with(|active| {
                active
                    .borrow()
                    .iter()
                    .any(|variable| variable.owner == self.owner && *variable.name == *self.name)
            })
    }
}

impl Type {
    /// A type variable of the generic body being checked: a real type, related
    /// through its constraint, not an unmodelled hole.
    pub fn is_type_variable(&self) -> bool {
        matches!(self, Type::TypeParameter(parameter) if parameter.is_active_variable())
    }

    /// [`Type::is_unknown`] without the type variables of the body being
    /// checked: what a relation must not judge.
    pub fn is_unmodelled(&self) -> bool {
        self.is_unknown() && !self.is_type_variable()
    }
}

/// Whether `ty` mentions a type variable of a body being checked. Such a
/// variable is identified by its owner, which a key that renders the type by
/// name does not see: two bodies' `T` look alike.
pub fn mentions_type_variable(ty: &Type) -> bool {
    fn walk(ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let nested = |ty: &Type| walk(ty, depth + 1);
        match ty {
            Type::TypeParameter(parameter) => parameter.owner != 0,
            Type::Array(element) => nested(element),
            Type::Tuple(elements) => elements.iter().any(nested),
            Type::Union(union) => union.types().iter().any(nested),
            Type::Function(function) => function.parameters().iter().any(nested) || nested(function.return_type()),
            Type::Object(object) => {
                object.properties.values().any(|property| nested(&property.ty))
                    || object.string_index_type.as_deref().is_some_and(nested)
                    || object.number_index_type.as_deref().is_some_and(nested)
                    || object.intersection_operands.as_deref().is_some_and(|operands| operands.iter().any(nested))
            }
            Type::Reference(reference) => reference.arguments.iter().any(nested),
            _ => false,
        }
    }
    walk(ty, 0)
}

/// `variable & with`, as narrowing a type variable produces it (`T & string`
/// under `typeof`, `T & C` under `instanceof`): a memberless intersection
/// whose operands are the whole type. The relation reads the operands, and a
/// primitive operand answers member reads.
pub fn intersect_type_variable(variable: &Type, with: Type) -> Type {
    type_variable_intersection(vec![variable.clone(), with])
}

/// An intersection with at least one type variable operand, kept as its
/// operands rather than merged: tsc keeps `T & X` generic, and merging would
/// drop the variable (surge's merge treats it as an unmodelled operand).
pub fn type_variable_intersection(operands: Vec<Type>) -> Type {
    Type::Object(
        crate::ObjectType::new(Default::default(), None)
            .with_intersection_marker()
            .with_intersection_operands(operands),
    )
}

/// A type variable narrowed to `T & X` (see [`intersect_type_variable`]).
pub fn is_narrowed_type_variable(ty: &Type) -> bool {
    matches!(ty, Type::Object(object)
        if object.is_intersection
            && object.properties.is_empty()
            && object.string_index_type.is_none()
            && object
                .intersection_operands
                .as_deref()
                .is_some_and(|operands| operands.iter().any(|operand| matches!(operand, Type::TypeParameter(_)))))
}

/// A type variable narrowed to `T & null` or `T & undefined`: definitely falsy.
pub fn is_nullish_type_variable_intersection(ty: &Type) -> bool {
    matches!(ty, Type::Object(object)
        if object.is_intersection
            && object.properties.is_empty()
            && object.intersection_operands.as_deref().is_some_and(|operands| {
                operands.iter().any(|operand| matches!(operand, Type::TypeParameter(_)))
                    && operands.iter().any(|operand| matches!(operand, Type::Null | Type::Undefined | Type::Void))
            }))
}

/// tsc's `getGlobalNonNullableTypeInstantiation` for a type variable whose
/// constraint may be nullish: `T` proven non-nullish (truthy, `!`, `?.`) is
/// `T & {}`, which keeps its identity and relates through the constraint left
/// once nullish members are gone. Anything else is returned as it is.
pub fn non_nullable_type_variable(ty: &Type) -> Type {
    let Type::TypeParameter(parameter) = ty else {
        return ty.clone();
    };
    let Some(constraint) = active_constraint(parameter) else {
        return ty.clone();
    };
    let may_be_nullish = match &constraint {
        None => true,
        Some(constraint) if constraint.is_unknown() => true,
        Some(Type::Union(union)) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Null | Type::Undefined | Type::Void) || member.is_unknown()),
        Some(other) => matches!(other, Type::Null | Type::Undefined | Type::Void),
    };
    if !may_be_nullish || !crate::strict_null_checks() {
        return ty.clone();
    }
    intersect_type_variable(ty, Type::Object(crate::ObjectType::new(Default::default(), None)))
}
