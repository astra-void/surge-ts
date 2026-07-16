use std::fmt;
use std::sync::Arc;

use crate::Type;

/// Lazily computes the structural expansion of a [`TypeReference`].
///
/// `surge-ts-types` is a leaf crate with no access to the checker's declaration
/// table, so the checker installs the concrete resolver behind this trait.
/// Implementations are expected to memoize program-wide (one structural
/// instantiation per unique declaration + type-argument tuple) so repeated
/// `resolve` calls are cheap.
pub trait ResolveReference: Send + Sync {
    fn resolve(&self) -> Type;

    fn retains_resolution_context(&self) -> bool {
        false
    }

    fn supports_program_canonicalization(&self) -> bool {
        true
    }

    fn program_canonicalization_discriminator(&self) -> u64 {
        0
    }

    /// Like [`resolve`](Self::resolve) but yields a shared `Arc<Type>`. Resolvers
    /// backed by a memoized/interned `Arc` (the lazy/interned instantiation
    /// resolvers) override this to hand back the shared pointer instead of
    /// deep-cloning the structural type. The assignability checker peels the same
    /// reference millions of times on conditional/mapped-type-heavy programs and
    /// only needs to *borrow* the resolved shape, so avoiding the per-peel `Type`
    /// clone is a large win; the default keeps the old behaviour for resolvers
    /// that own their type by value.
    fn resolve_arc(&self) -> Arc<Type> {
        Arc::new(self.resolve())
    }
}

/// A lazy, nominal reference to a named type instantiation (`Box<string>`,
/// `User`, …), mirroring tsc's `TypeReference`. Construction is cheap: the
/// structural shape is computed on demand via [`ResolveReference::resolve`] and
/// memoized by the checker, instead of being eagerly expanded at every use site.
///
/// Equality is **nominal** — two references are equal when they come from the
/// same declaration ([`id`](Self::id)) with equal [`arguments`](Self::arguments).
/// The `display` string and the resolver identity are not part of equality, so a
/// reference never compares equal to its own structural expansion (which is the
/// correct nominal semantics) and same-instantiation references compare equal in
/// O(1) without forcing resolution.
#[derive(Clone)]
pub struct TypeReference {
    /// Nominal identity of the source declaration (qualified `file\0Name`).
    pub id: Arc<str>,
    /// Diagnostic display form, e.g. `Box<string>` or `User`.
    pub display: Arc<str>,
    /// Resolved type arguments, for variance-aware comparison and display.
    pub arguments: Arc<[Type]>,
    resolver: Arc<dyn ResolveReference>,
}

impl TypeReference {
    pub fn new(
        id: impl Into<Arc<str>>,
        display: impl Into<Arc<str>>,
        arguments: impl Into<Arc<[Type]>>,
        resolver: Arc<dyn ResolveReference>,
    ) -> Self {
        Self {
            id: id.into(),
            display: display.into(),
            arguments: arguments.into(),
            resolver,
        }
    }

    /// Computes (or returns the memoized) structural expansion of this reference.
    pub fn resolve(&self) -> Type {
        self.resolver.resolve()
    }

    /// The structural expansion as a shared `Arc<Type>`, avoiding a deep clone
    /// when the underlying resolver is `Arc`-backed. See
    /// [`ResolveReference::resolve_arc`].
    pub fn resolve_arc(&self) -> Arc<Type> {
        self.resolver.resolve_arc()
    }

    pub fn retains_resolution_context(&self) -> bool {
        self.resolver.retains_resolution_context()
    }

    pub fn supports_program_canonicalization(&self) -> bool {
        self.resolver.supports_program_canonicalization()
    }

    pub fn program_canonicalization_discriminator(&self) -> u64 {
        self.resolver.program_canonicalization_discriminator()
    }

    pub fn resolver_address(&self) -> usize {
        Arc::as_ptr(&self.resolver) as *const () as usize
    }

    /// Nominal identity test: same declaration and same type arguments.
    pub fn nominal_eq(&self, other: &Self) -> bool {
        self.id == other.id && self.arguments == other.arguments
    }
}

impl fmt::Debug for TypeReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypeReference")
            .field("id", &self.id)
            .field("display", &self.display)
            .field("arguments", &self.arguments)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TypeReference {
    fn eq(&self, other: &Self) -> bool {
        self.nominal_eq(other)
    }
}

impl Eq for TypeReference {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::is_assignable_to;

    struct Fixed(Type);

    impl ResolveReference for Fixed {
        fn resolve(&self) -> Type {
            self.0.clone()
        }
    }

    fn reference(id: &str, display: &str, arguments: Vec<Type>, resolved: Type) -> Type {
        Type::Reference(TypeReference::new(
            id,
            display,
            arguments,
            Arc::new(Fixed(resolved)),
        ))
    }

    #[test]
    fn name_uses_display_not_structural_expansion() {
        let ty = reference(
            "box.ts\u{0}Box",
            "Box<string>",
            vec![Type::String],
            Type::String,
        );
        assert_eq!(ty.name(), "Box<string>");
    }

    #[test]
    fn same_declaration_and_arguments_are_nominally_equal() {
        let a = reference(
            "box.ts\u{0}Box",
            "Box<string>",
            vec![Type::String],
            Type::String,
        );
        let b = reference(
            "box.ts\u{0}Box",
            "Box<string> (other display)",
            vec![Type::String],
            Type::Number,
        );
        // Equality is nominal: identical id + arguments, regardless of display or
        // resolver identity.
        assert_eq!(a, b);
        assert!(is_assignable_to(&a, &b));
    }

    #[test]
    fn different_arguments_are_not_nominally_equal() {
        let a = reference(
            "box.ts\u{0}Box",
            "Box<string>",
            vec![Type::String],
            Type::String,
        );
        let b = reference(
            "box.ts\u{0}Box",
            "Box<number>",
            vec![Type::Number],
            Type::Number,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn reference_falls_back_to_structural_expansion_for_assignability() {
        // `type Id = string;` — a reference whose structural form is `string`.
        let id = reference("ids.ts\u{0}Id", "Id", vec![], Type::String);
        assert!(is_assignable_to(&id, &Type::String));
        assert!(is_assignable_to(&Type::StringLiteral("x".to_string()), &id));
        assert!(!is_assignable_to(&id, &Type::Number));
    }

    #[test]
    fn reference_resolves_for_base_primitive() {
        let id = reference("ids.ts\u{0}Id", "Id", vec![], Type::String);
        assert_eq!(id.base_primitive(), Some(Type::String));
    }
}
