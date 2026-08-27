use std::collections::HashMap;
use std::sync::Arc;

use surge_ts_syntax::{
    ParsedFunctionType, ParsedInterfaceMember, ParsedNamedType, ParsedType, ParsedTypeParameter,
    TextSpan,
};

/// The heavy, immutable payload of a type alias declaration.
///
/// Held behind an `Arc` in [`TypeAliasInfo`] so that re-export/import rebinding,
/// which only rewrites the per-binding header (`name`, `resolution_scope`), shares
/// the parsed alias tree by pointer instead of deep-cloning it. The body is never
/// mutated after declaration collection; declaration merging builds a fresh body.
#[derive(Debug)]
pub(crate) struct TypeAliasBody {
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) ty: ParsedType,
}

impl Clone for TypeAliasBody {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_payload_deep_clone_count();
        Self {
            type_parameters: self.type_parameters.clone(),
            ty: self.ty.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct TypeAliasInfo {
    pub(crate) name: Arc<str>,
    /// The name this declaration was originally declared under at its source,
    /// captured the first time it is renamed by a re-export/import (`import type
    /// { Box as ABox }`). Used only for diagnostic display so messages show the
    /// original name (`Box<string>`) rather than the local binding (`ABox`).
    pub(crate) declared_name: Option<Arc<str>>,
    pub(crate) file_name: Arc<str>,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) body: Arc<TypeAliasBody>,
    /// Memoized resolution-cache key (canonical file name + declared name).
    /// Built on first request — key construction canonicalizes the path and
    /// allocates, and resolution asks for it millions of times per run. Carried
    /// across clones (the key is a pure function of `file_name`/`name`) and
    /// reset by `rename_type_declaration`, the only place that rewrites `name`.
    pub(crate) cached_resolution_key: std::sync::OnceLock<crate::context::DeclarationResolutionKey>,
    /// Memoized nominal reference id (`"{canonical file}\0{name}"`), the other
    /// per-resolution allocation on the named-type hot path. Same lifecycle as
    /// [`Self::cached_resolution_key`].
    pub(crate) cached_alias_id: std::sync::OnceLock<Arc<str>>,
}

impl TypeAliasInfo {
    pub(crate) fn new(
        name: impl Into<Arc<str>>,
        file_name: impl Into<Arc<str>>,
        name_span: Option<TextSpan>,
        type_parameters: Vec<ParsedTypeParameter>,
        ty: ParsedType,
        resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ) -> Self {
        Self {
            name: name.into(),
            declared_name: None,
            file_name: file_name.into(),
            name_span,
            resolution_scope,
            body: Arc::new(TypeAliasBody {
                type_parameters,
                ty,
            }),
            cached_resolution_key: std::sync::OnceLock::new(),
            cached_alias_id: std::sync::OnceLock::new(),
        }
    }
}

impl Clone for TypeAliasInfo {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_header_copy_count();
        Self {
            name: self.name.clone(),
            declared_name: self.declared_name.clone(),
            file_name: self.file_name.clone(),
            name_span: self.name_span,
            resolution_scope: self.resolution_scope.clone(),
            body: self.body.clone(),
            cached_resolution_key: self.cached_resolution_key.clone(),
            cached_alias_id: self.cached_alias_id.clone(),
        }
    }
}

/// The heavy, immutable payload of an interface declaration. See [`TypeAliasBody`]
/// for the body/header sharing rationale.
#[derive(Debug)]
pub(crate) struct InterfaceBody {
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) extends: Vec<ParsedNamedType>,
    pub(crate) members: Vec<ParsedInterfaceMember>,
    pub(crate) string_index_type: Option<ParsedType>,
    pub(crate) call_signature: Option<ParsedFunctionType>,
    pub(crate) construct_signatures: Vec<ParsedFunctionType>,
    pub(crate) declaration_fragments: Vec<InterfaceDeclarationFragmentId>,
    pub(crate) member_fragments: Vec<InterfaceDeclarationFragmentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceDeclarationFragmentId {
    /// Shared: every member fragment of one declaration points at the same
    /// file string (a per-member owned copy multiplied a path across every
    /// interface member of every dependency `.d.ts`).
    pub(crate) file_name: Arc<str>,
    pub(crate) declaration_start: usize,
}

impl Clone for InterfaceBody {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_payload_deep_clone_count();
        Self {
            type_parameters: self.type_parameters.clone(),
            extends: self.extends.clone(),
            members: self.members.clone(),
            string_index_type: self.string_index_type.clone(),
            call_signature: self.call_signature.clone(),
            construct_signatures: self.construct_signatures.clone(),
            declaration_fragments: self.declaration_fragments.clone(),
            member_fragments: self.member_fragments.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct InterfaceInfo {
    pub(crate) name: Arc<str>,
    /// See [`TypeAliasInfo::declared_name`]. Original source name, captured on the
    /// first re-export/import rename, for diagnostic display only.
    pub(crate) declared_name: Option<Arc<str>>,
    pub(crate) file_name: Arc<str>,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) body: Arc<InterfaceBody>,
    /// See [`TypeAliasInfo::cached_resolution_key`].
    pub(crate) cached_resolution_key: std::sync::OnceLock<crate::context::DeclarationResolutionKey>,
    /// See [`TypeAliasInfo::cached_alias_id`].
    pub(crate) cached_alias_id: std::sync::OnceLock<Arc<str>>,
    /// Memoized stable declaration identity for the instantiation cache
    /// (`None` = the declaration is unstable and never cacheable). Depends on
    /// `file_name`, `name_span`, the declared name, and every merged fragment,
    /// so it must be reset wherever `declaration_fragments` grows on a cloned
    /// accumulator and wherever the declaration is renamed.
    pub(crate) cached_stable_id:
        std::sync::OnceLock<Option<crate::context::StableInterfaceDeclarationId>>,
}

impl InterfaceInfo {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        name: impl Into<Arc<str>>,
        file_name: impl Into<Arc<str>>,
        name_span: Option<TextSpan>,
        type_parameters: Vec<ParsedTypeParameter>,
        extends: Vec<ParsedNamedType>,
        members: Vec<ParsedInterfaceMember>,
        string_index_type: Option<ParsedType>,
        call_signature: Option<ParsedFunctionType>,
        construct_signatures: Vec<ParsedFunctionType>,
        resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ) -> Self {
        let file_name = file_name.into();
        let declaration_fragment = InterfaceDeclarationFragmentId {
            file_name: file_name.clone(),
            declaration_start: name_span.map_or(0, |span| span.start),
        };
        let member_fragments = vec![declaration_fragment.clone(); members.len()];
        Self {
            name: name.into(),
            declared_name: None,
            file_name,
            name_span,
            resolution_scope,
            body: Arc::new(InterfaceBody {
                type_parameters,
                extends,
                members,
                string_index_type,
                call_signature,
                construct_signatures,
                declaration_fragments: vec![declaration_fragment],
                member_fragments,
            }),
            cached_resolution_key: std::sync::OnceLock::new(),
            cached_alias_id: std::sync::OnceLock::new(),
            cached_stable_id: std::sync::OnceLock::new(),
        }
    }
}

impl Clone for InterfaceInfo {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_header_copy_count();
        Self {
            name: self.name.clone(),
            declared_name: self.declared_name.clone(),
            file_name: self.file_name.clone(),
            name_span: self.name_span,
            resolution_scope: self.resolution_scope.clone(),
            body: self.body.clone(),
            cached_resolution_key: self.cached_resolution_key.clone(),
            cached_alias_id: self.cached_alias_id.clone(),
            cached_stable_id: self.cached_stable_id.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum TypeDeclarationInfo {
    Alias(TypeAliasInfo),
    Interface(InterfaceInfo),
}

impl Clone for TypeDeclarationInfo {
    fn clone(&self) -> Self {
        match self {
            Self::Alias(info) => Self::Alias(info.clone()),
            Self::Interface(info) => Self::Interface(info.clone()),
        }
    }
}

impl TypeDeclarationInfo {
    /// The name this declaration was originally declared under at its source,
    /// falling back to its current binding name when it was never renamed. Used
    /// for diagnostic display so an import-renamed type shows its original name
    /// (`Box`) rather than the local binding (`ABox`).
    pub(crate) fn declared_name(&self) -> &str {
        match self {
            Self::Alias(info) => info.declared_name.as_deref().unwrap_or(&info.name),
            Self::Interface(info) => info.declared_name.as_deref().unwrap_or(&info.name),
        }
    }
}

/// Merge two interface declarations following TypeScript declaration merging:
/// the existing members come first, then the incoming members. `extends` clauses
/// concatenate. Same-named method members are preserved (overloads); a property
/// already declared by the existing interface wins (matching TypeScript, which
/// keeps the first declaration's type), so a later conflicting property is
/// dropped here. Property conflict *reporting* is the caller's responsibility.
pub(crate) fn merge_interface_infos(
    existing: &InterfaceInfo,
    incoming: &InterfaceInfo,
) -> InterfaceInfo {
    let is_method = |member: &ParsedInterfaceMember| matches!(member.ty, ParsedType::Function(_));
    let existing_property_names: std::collections::HashSet<&str> = existing
        .body
        .members
        .iter()
        .filter(|member| !is_method(member))
        .map(|member| member.name.as_str())
        .collect();

    let mut members = existing.body.members.clone();
    let mut member_fragments = existing.body.member_fragments.clone();
    for (member, fragment) in incoming
        .body
        .members
        .iter()
        .zip(incoming.body.member_fragments.iter())
    {
        if !is_method(member) && existing_property_names.contains(member.name.as_str()) {
            continue;
        }
        members.push(member.clone());
        member_fragments.push(fragment.clone());
    }
    let mut extends = existing.body.extends.clone();
    extends.extend(incoming.body.extends.iter().cloned());
    let type_parameters = if existing.body.type_parameters.is_empty() {
        incoming.body.type_parameters.clone()
    } else {
        existing.body.type_parameters.clone()
    };
    let mut merged_info = InterfaceInfo::new(
        existing.name.clone(),
        existing.file_name.clone(),
        existing.name_span,
        type_parameters,
        extends,
        members,
        existing
            .body
            .string_index_type
            .clone()
            .or_else(|| incoming.body.string_index_type.clone()),
        existing
            .body
            .call_signature
            .clone()
            .or_else(|| incoming.body.call_signature.clone()),
        {
            let mut merged = existing.body.construct_signatures.clone();
            merged.extend(incoming.body.construct_signatures.iter().cloned());
            merged
        },
        existing
            .resolution_scope
            .clone()
            .or_else(|| incoming.resolution_scope.clone()),
    );
    Arc::make_mut(&mut merged_info.body).declaration_fragments = existing
        .body
        .declaration_fragments
        .iter()
        .chain(incoming.body.declaration_fragments.iter())
        .cloned()
        .collect();
    Arc::make_mut(&mut merged_info.body).member_fragments = member_fragments;
    merged_info
}

/// Insert `incoming` into `table`, merging into an existing interface of the same
/// name (declaration merging) rather than dropping the later declaration.
/// Non-interface collisions keep the first declaration (first-wins), matching a
/// `var`/interface pair where the interface supplies the type.
pub(crate) fn merge_type_declaration_into_table(
    table: &mut TypeDeclarationTable,
    name: &str,
    incoming: &TypeDeclarationInfo,
) {
    enum Action {
        Merge(Box<InterfaceInfo>),
        KeepFirst,
        Insert,
    }

    let action = match (table.get(name), incoming) {
        (
            Some(TypeDeclarationInfo::Interface(existing)),
            TypeDeclarationInfo::Interface(incoming),
        ) => Action::Merge(Box::new(merge_interface_infos(existing, incoming))),
        (Some(_), _) => Action::KeepFirst,
        (None, _) => Action::Insert,
    };

    match action {
        Action::Merge(merged) => table.upsert(name, TypeDeclarationInfo::Interface(*merged)),
        Action::KeepFirst => {}
        Action::Insert => {
            let _ = table.insert(name, incoming.clone());
        }
    }
}

/// Merge every declaration of `source` into `dest`, sharing unchanged payloads
/// and allocating a fresh payload only for merged interfaces.
pub(crate) fn merge_shared_table_into(
    dest: &mut TypeDeclarationTable,
    source: &TypeDeclarationTable,
) {
    enum Action {
        Merge(Box<InterfaceInfo>),
        KeepFirst,
        Share(Arc<TypeDeclarationInfo>),
    }

    for (name, incoming) in source.declarations.iter() {
        let name_ref = name.as_ref();
        let action = match (dest.get(name_ref), incoming.as_ref()) {
            (
                Some(TypeDeclarationInfo::Interface(existing)),
                TypeDeclarationInfo::Interface(incoming),
            ) => Action::Merge(Box::new(merge_interface_infos(existing, incoming))),
            (Some(_), _) => Action::KeepFirst,
            (None, _) => Action::Share(incoming.clone()),
        };

        match action {
            Action::Merge(merged) => dest.upsert(name_ref, TypeDeclarationInfo::Interface(*merged)),
            Action::KeepFirst => {}
            Action::Share(declaration) => dest.insert_shared_handle(name.clone(), declaration),
        }
    }
}

/// Bulk variant of [`merge_shared_table_into`] that merges many sources into
/// `dest` in one pass. Applying the single-source merge once per source rebuilds a
/// growing interface's member list on every merge, so a global interface split
/// across N files (`declare global { interface X { ... } }`) was O(N^2). Here each
/// interface is folded into one owned accumulator whose property set is maintained
/// incrementally, making the whole merge O(total members).
pub(crate) fn merge_shared_tables_into(
    dest: &mut TypeDeclarationTable,
    sources: &[TypeDeclarationTable],
) {
    let is_method = |member: &ParsedInterfaceMember| matches!(member.ty, ParsedType::Function(_));
    // name -> (accumulated interface, its non-method property names)
    let mut interfaces: HashMap<String, (InterfaceInfo, std::collections::HashSet<String>)> =
        HashMap::new();

    for source in sources {
        for (name, incoming) in source.declarations.iter() {
            let name_ref = name.as_ref();

            let TypeDeclarationInfo::Interface(incoming_interface) = incoming.as_ref() else {
                // A later non-interface never overrides an earlier binding (first
                // wins), and an interface already accumulated for this name keeps it.
                if dest.get(name_ref).is_some() || interfaces.contains_key(name_ref) {
                    continue;
                }
                dest.insert_shared_handle(name.clone(), incoming.clone());
                continue;
            };

            if let Some((accumulator, seen)) = interfaces.get_mut(name_ref) {
                fold_interface_declaration(accumulator, seen, incoming_interface, &is_method);
                continue;
            }

            match dest.get(name_ref) {
                Some(TypeDeclarationInfo::Interface(existing)) => {
                    let mut accumulator = existing.clone();
                    let mut seen: std::collections::HashSet<String> = existing
                        .body
                        .members
                        .iter()
                        .filter(|member| !is_method(member))
                        .map(|member| member.name.clone())
                        .collect();
                    fold_interface_declaration(
                        &mut accumulator,
                        &mut seen,
                        incoming_interface,
                        &is_method,
                    );
                    interfaces.insert(name_ref.to_string(), (accumulator, seen));
                }
                // An existing non-interface binding wins over a later interface.
                Some(_) => {}
                None => {
                    let seen: std::collections::HashSet<String> = incoming_interface
                        .body
                        .members
                        .iter()
                        .filter(|member| !is_method(member))
                        .map(|member| member.name.clone())
                        .collect();
                    interfaces.insert(name_ref.to_string(), (incoming_interface.clone(), seen));
                }
            }
        }
    }

    for (name, (accumulator, _)) in interfaces {
        dest.upsert(&name, TypeDeclarationInfo::Interface(accumulator));
    }
}

/// Append `incoming`'s members to `accumulator` in place, mirroring
/// [`merge_interface_infos`] (methods always kept as overloads; a property already
/// declared wins) but reusing the accumulator's member list instead of cloning it,
/// and consulting an incrementally maintained `seen` set instead of rebuilding it.
fn fold_interface_declaration(
    accumulator: &mut InterfaceInfo,
    seen: &mut std::collections::HashSet<String>,
    incoming: &InterfaceInfo,
    is_method: &impl Fn(&ParsedInterfaceMember) -> bool,
) {
    // The accumulator is a clone of an existing declaration, so a memoized
    // stable id would go stale as the fragment list grows below.
    accumulator.cached_stable_id = std::sync::OnceLock::new();
    let body = Arc::make_mut(&mut accumulator.body);
    for (member, fragment) in incoming
        .body
        .members
        .iter()
        .zip(incoming.body.member_fragments.iter())
    {
        if !is_method(member) {
            if seen.contains(member.name.as_str()) {
                continue;
            }
            seen.insert(member.name.clone());
        }
        body.members.push(member.clone());
        body.member_fragments.push(fragment.clone());
    }
    body.extends.extend(incoming.body.extends.iter().cloned());
    if body.type_parameters.is_empty() {
        body.type_parameters = incoming.body.type_parameters.clone();
    }
    if body.string_index_type.is_none() {
        body.string_index_type = incoming.body.string_index_type.clone();
    }
    if body.call_signature.is_none() {
        body.call_signature = incoming.body.call_signature.clone();
    }
    body.construct_signatures
        .extend(incoming.body.construct_signatures.iter().cloned());
    body.declaration_fragments
        .extend(incoming.body.declaration_fragments.iter().cloned());
    if accumulator.resolution_scope.is_none() {
        accumulator.resolution_scope = incoming.resolution_scope.clone();
    }
}

#[derive(Clone)]
pub(crate) struct TypeDeclarationHandle {
    declaration: Arc<TypeDeclarationInfo>,
}

impl TypeDeclarationHandle {
    pub(crate) fn get(&self) -> &TypeDeclarationInfo {
        self.declaration.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TypeDeclarationScope {
    layers: Vec<Arc<TypeDeclarationTable>>,
}

impl TypeDeclarationScope {
    pub(crate) fn new(layers: Vec<Arc<TypeDeclarationTable>>) -> Self {
        Self { layers }
    }

    /// Census-only: layer table identities, for the scope↔table cycle probe.
    pub(crate) fn census_layer_addresses(&self) -> impl Iterator<Item = usize> + '_ {
        self.layers.iter().map(|layer| layer.identity_address())
    }

    pub(crate) fn get(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        let mut layers_visited = 0;

        for layer in &self.layers {
            layers_visited += 1;
            if let Some(declaration) = layer.get_without_lookup_record(name) {
                crate::program::record_declaration_lookup(layers_visited);
                return Some(declaration);
            }
        }

        crate::program::record_declaration_lookup(layers_visited);
        None
    }

    pub(crate) fn get_handle(&self, name: &str) -> Option<TypeDeclarationHandle> {
        let mut layers_visited = 0;

        for layer in &self.layers {
            layers_visited += 1;
            if let Some(handle) = layer.get_handle(name) {
                crate::program::record_declaration_lookup(layers_visited);
                return Some(handle);
            }
        }

        crate::program::record_declaration_lookup(layers_visited);
        None
    }

    #[allow(dead_code)]
    pub(crate) fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub(crate) fn layers(&self) -> &[Arc<TypeDeclarationTable>] {
        &self.layers
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.layers.iter().all(|layer| layer.len() == 0)
    }
}

#[derive(Debug)]
/// Shared top-level type-declaration namespace for aliases and interfaces.
///
/// The first declaration wins; later duplicates are reported by the caller and
/// must not replace the original entry.
pub(crate) struct TypeDeclarationTable {
    declarations:
        Arc<surge_ts_types::fx::FxHashMap<Arc<str>, Arc<TypeDeclarationInfo>>>,
    /// Instance identity + mutation counter. `(instance_id, version)` equality
    /// proves this exact table instance is bytewise-unchanged since a previous
    /// observation, letting the declaration-environment capture reuse one
    /// immutable snapshot across the many environments interned between
    /// mutations. Clones get a fresh identity (they diverge independently).
    instance_id: u64,
    version: u64,
}

fn next_table_instance_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Default for TypeDeclarationTable {
    fn default() -> Self {
        Self {
            declarations: Arc::new(Default::default()),
            instance_id: next_table_instance_id(),
            version: 0,
        }
    }
}

impl Clone for TypeDeclarationTable {
    fn clone(&self) -> Self {
        Self {
            declarations: self.declarations.clone(),
            instance_id: next_table_instance_id(),
            version: 0,
        }
    }
}

impl TypeDeclarationTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// See the `instance_id` field: equal pairs prove an unchanged instance.
    pub(crate) fn snapshot_identity(&self) -> (u64, u64) {
        (self.instance_id, self.version)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        crate::program::record_declaration_lookup(1);
        self.get_without_lookup_record(name)
    }

    fn get_without_lookup_record(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        self.declarations.get(name).map(Arc::as_ref)
    }

    pub(crate) fn get_handle(&self, name: &str) -> Option<TypeDeclarationHandle> {
        Some(TypeDeclarationHandle {
            declaration: self.declarations.get(name)?.clone(),
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Census-only identity of the shared COW map backing this table.
    pub(crate) fn identity_address(&self) -> usize {
        Arc::as_ptr(&self.declarations) as usize
    }

    /// Census-only estimate of this map backing's owned index memory.
    pub(crate) fn index_heap_bytes(&self) -> u64 {
        (self.declarations.capacity()
            * (std::mem::size_of::<Arc<str>>()
                + std::mem::size_of::<Arc<TypeDeclarationInfo>>()
                + 16)) as u64
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &TypeDeclarationInfo)> + '_ {
        self.declarations
            .iter()
            .map(|(name, declaration)| (name, declaration.as_ref()))
    }

    pub(crate) fn insert(
        &mut self,
        name: impl AsRef<str>,
        declaration: TypeDeclarationInfo,
    ) -> Option<TypeDeclarationInfo> {
        let name_ref = name.as_ref();
        if self.declarations.contains_key(name_ref) {
            return Some(declaration);
        }

        let key = declaration_key(name_ref, &declaration);
        Arc::make_mut(&mut self.declarations).insert(key, Arc::new(declaration));
        self.version += 1;
        None
    }

    /// Insert `declaration`, replacing any existing entry for `name`.
    ///
    /// Unlike [`insert`](Self::insert) (which is first-wins), this overwrites the
    /// existing payload. Used by declaration-merging paths.
    pub(crate) fn upsert(&mut self, name: impl AsRef<str>, declaration: TypeDeclarationInfo) {
        let name_ref = name.as_ref();
        let declarations = Arc::make_mut(&mut self.declarations);
        if let Some(existing) = declarations.get_mut(name_ref) {
            *existing = Arc::new(declaration);
        } else {
            let key = declaration_key(name_ref, &declaration);
            declarations.insert(key, Arc::new(declaration));
        }
        self.version += 1;
    }

    fn insert_shared_handle(
        &mut self,
        name: Arc<str>,
        declaration: Arc<TypeDeclarationInfo>,
    ) {
        if self.declarations.contains_key(name.as_ref()) {
            return;
        }
        Arc::make_mut(&mut self.declarations).insert(name, declaration);
        self.version += 1;
    }

    /// Share `source`'s payload for `source_name` under `name`, first-wins,
    /// without cloning the declaration. This is the per-importer qualified
    /// namespace binding path, where the declaration content is byte-identical.
    pub(crate) fn insert_shared_from(
        &mut self,
        name: &str,
        source: &TypeDeclarationTable,
        source_name: &str,
    ) -> bool {
        if self.declarations.contains_key(name) {
            return false;
        }
        let Some(declaration) = source.declarations.get(source_name).cloned() else {
            return false;
        };
        self.insert_shared_handle(Arc::from(name), declaration);
        true
    }
}

fn declaration_key(name: &str, declaration: &TypeDeclarationInfo) -> Arc<str> {
    let declared_name = match declaration {
        TypeDeclarationInfo::Alias(alias) => &alias.name,
        TypeDeclarationInfo::Interface(interface) => &interface.name,
    };
    if declared_name.as_ref() == name {
        declared_name.clone()
    } else {
        Arc::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_declaration_table_is_first_wins_across_kinds() {
        let mut table = TypeDeclarationTable::new();

        let alias = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            "User".to_string(),
            "a.ts".to_string(),
            None,
            vec![],
            ParsedType::String,
            None,
        ));
        let interface = TypeDeclarationInfo::Interface(InterfaceInfo::new(
            "User".to_string(),
            "b.ts".to_string(),
            None,
            vec![],
            vec![],
            vec![],
            None,
            None,
            Vec::new(),
            None,
        ));

        assert!(table.insert("User", alias.clone()).is_none());
        assert!(table.insert("User", interface.clone()).is_some());
        assert!(matches!(
            table.get("User"),
            Some(TypeDeclarationInfo::Alias(_))
        ));

        let mut table = TypeDeclarationTable::new();
        assert!(table.insert("User", interface.clone()).is_none());
        assert!(table.insert("User", alias).is_some());
        assert!(matches!(
            table.get("User"),
            Some(TypeDeclarationInfo::Interface(_))
        ));
    }

    fn sample_alias(name: &str, file: &str) -> TypeDeclarationInfo {
        TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            name.to_string(),
            file.to_string(),
            None,
            vec![],
            ParsedType::String,
            None,
        ))
    }

    #[test]
    fn snapshot_identity_tracks_mutations_and_never_aliases_instances() {
        let mut table = TypeDeclarationTable::new();
        let unmodified = table.snapshot_identity();

        let _ = table.insert("User", sample_alias("User", "a.ts"));
        let mutated = table.snapshot_identity();
        assert_eq!(unmodified.0, mutated.0);
        assert_ne!(unmodified.1, mutated.1);

        let mut clone = table.clone();
        assert_ne!(clone.snapshot_identity().0, table.snapshot_identity().0);
        let _ = clone.insert("Other", sample_alias("Other", "b.ts"));
        assert_eq!(table.snapshot_identity(), mutated);
    }

    #[test]
    fn shared_payload_stays_valid_after_source_table_drops() {
        let mut source = TypeDeclarationTable::new();
        let _ = source.insert("User", sample_alias("User", "exporter.d.ts"));

        let mut importer = TypeDeclarationTable::new();
        assert!(importer.insert_shared_from("Renamed", &source, "User"));
        assert!(!importer.insert_shared_from("Renamed", &source, "User"));

        drop(source);
        assert!(matches!(
            importer.get("Renamed"),
            Some(TypeDeclarationInfo::Alias(_))
        ));

        let handle = importer.get_handle("Renamed").expect("shared entry");
        drop(importer);
        assert!(matches!(handle.get(), TypeDeclarationInfo::Alias(_)));
    }

    #[test]
    fn type_declaration_scope_honors_layer_precedence() {
        let mut ambient = TypeDeclarationTable::new();
        let ambient_alias = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            "User".to_string(),
            "ambient.d.ts".to_string(),
            None,
            vec![],
            ParsedType::String,
            None,
        ));
        let _ = ambient.insert("User", ambient_alias.clone());

        let mut local = TypeDeclarationTable::new();
        let local_interface = TypeDeclarationInfo::Interface(InterfaceInfo::new(
            "User".to_string(),
            "local.ts".to_string(),
            None,
            vec![],
            vec![],
            vec![],
            None,
            None,
            Vec::new(),
            None,
        ));
        let _ = local.insert("User", local_interface.clone());

        let scope = TypeDeclarationScope::new(vec![Arc::new(local), Arc::new(ambient)]);

        assert_eq!(scope.layer_count(), 2);
        assert!(matches!(
            scope.get("User"),
            Some(TypeDeclarationInfo::Interface(_))
        ));

        let mut ambient_only = TypeDeclarationTable::new();
        let ambient_only_alias = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            "Buffer".to_string(),
            "ambient.d.ts".to_string(),
            None,
            vec![],
            ParsedType::Unknown,
            None,
        ));
        let _ = ambient_only.insert("Buffer", ambient_only_alias);
        let scope = TypeDeclarationScope::new(vec![
            Arc::new(TypeDeclarationTable::new()),
            Arc::new(ambient_only),
        ]);
        assert!(matches!(
            scope.get("Buffer"),
            Some(TypeDeclarationInfo::Alias(_))
        ));
    }
}
