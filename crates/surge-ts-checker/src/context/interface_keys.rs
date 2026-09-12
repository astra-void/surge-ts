use std::hash::Hash;
use std::sync::Arc;
use super::{StableInterfaceDeclarationId, SubstitutionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceEnvironmentIdentity {
    pub(crate) no_lib: bool,
    pub(crate) skip_lib_check: bool,
    /// Content discriminator of the resolution environment the expansion was
    /// produced under — the same hash the canonical type store uses. Without it
    /// the key is `(declaration, arguments)` plus two run-constant booleans,
    /// which is the declaration-identity-only sharing the memory-lifetime rules
    /// forbid: the cache now covers every user interface during the check
    /// phase, where module scope, augmentation generation and the live
    /// declaration table all differ between consumers.
    pub(crate) environment_discriminator: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceInstantiationKey {
    pub(crate) declaration: StableInterfaceDeclarationId,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InterfaceMemberDeclarationKind {
    Property,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StableInterfaceMemberDeclarationId {
    pub(crate) containing_interface: StableInterfaceDeclarationId,
    pub(crate) canonical_file: Arc<str>,
    pub(crate) declaration_start: u32,
    pub(crate) declaration_kind: InterfaceMemberDeclarationKind,
    pub(crate) declared_name: Arc<str>,
    pub(crate) overload_index: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceMemberDeclarationTemplate {
    pub(crate) declaration: StableInterfaceMemberDeclarationId,
    pub(crate) overload_group: Option<u32>,
    pub(crate) overload_position: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceMethodOverloadGroupTemplate {
    pub(crate) ordered_members: Arc<[StableInterfaceMemberDeclarationId]>,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceDeclarationTemplate {
    pub(crate) members: Arc<[InterfaceMemberDeclarationTemplate]>,
    pub(crate) method_groups: Arc<[InterfaceMethodOverloadGroupTemplate]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceMemberInstantiationKey {
    pub(crate) member: StableInterfaceMemberDeclarationId,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceOverloadInstantiationKey {
    pub(crate) containing_interface: StableInterfaceDeclarationId,
    pub(crate) ordered_members: Arc<[StableInterfaceMemberDeclarationId]>,
    pub(crate) prefix_len: u32,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}
