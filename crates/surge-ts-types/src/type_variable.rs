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

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::{Type, TypeParameterType};

struct ActiveVariable {
    owner: u32,
    name: Arc<str>,
    declaration: (Arc<str>, u32),
    constraint: Option<Type>,
}

thread_local! {
    static ACTIVE_VARIABLES: RefCell<Vec<ActiveVariable>> = const { RefCell::new(Vec::new()) };
    static NEXT_OWNER: Cell<u32> = const { Cell::new(1) };
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
        let owner = NEXT_OWNER.with(|next| {
            let owner = next.get();
            next.set(owner.wrapping_add(1).max(1));
            owner
        });
        ACTIVE_VARIABLES.with(|active| {
            active.borrow_mut().extend(parameters.into_iter().map(|(name, declaration)| {
                ActiveVariable {
                    owner,
                    name,
                    declaration,
                    constraint: None,
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
            active
                .borrow_mut()
                .retain(|variable| !(variable.owner == self.owner && *variable.name == *name))
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
        ACTIVE_VARIABLES.with(|active| active.borrow_mut().retain(|variable| variable.owner != self.owner));
    }
}

/// The active variable a type-parameter *declaration* is bound to, if its
/// body is being checked.
pub fn variable_for_declaration(file: &str, name_offset: u32) -> Option<Type> {
    ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .rev()
            .find(|variable| *variable.declaration.0 == *file && variable.declaration.1 == name_offset)
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
pub fn active_constraint(parameter: &TypeParameterType) -> Option<Option<Type>> {
    if parameter.owner == 0 {
        return None;
    }
    ACTIVE_VARIABLES.with(|active| {
        active
            .borrow()
            .iter()
            .find(|variable| variable.owner == parameter.owner && *variable.name == *parameter.name)
            .map(|variable| variable.constraint.clone())
    })
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
