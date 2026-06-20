use std::collections::HashMap;
use std::sync::Arc;

use surge_ts_syntax::{
    ParsedFunctionType, ParsedInterfaceMember, ParsedNamedType, ParsedType, ParsedTypeParameter,
    TextSpan,
};

use crate::arena::{ArenaStr, CheckerArena};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TypeDeclarationId(u32);

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
    pub(crate) name: String,
    /// The name this declaration was originally declared under at its source,
    /// captured the first time it is renamed by a re-export/import (`import type
    /// { Box as ABox }`). Used only for diagnostic display so messages show the
    /// original name (`Box<string>`) rather than the local binding (`ABox`).
    pub(crate) declared_name: Option<String>,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) body: Arc<TypeAliasBody>,
}

impl TypeAliasInfo {
    pub(crate) fn new(
        name: String,
        file_name: String,
        name_span: Option<TextSpan>,
        type_parameters: Vec<ParsedTypeParameter>,
        ty: ParsedType,
        resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ) -> Self {
        Self {
            name,
            declared_name: None,
            file_name,
            name_span,
            resolution_scope,
            body: Arc::new(TypeAliasBody {
                type_parameters,
                ty,
            }),
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
        }
    }
}

#[derive(Debug)]
pub(crate) struct InterfaceInfo {
    pub(crate) name: String,
    /// See [`TypeAliasInfo::declared_name`]. Original source name, captured on the
    /// first re-export/import rename, for diagnostic display only.
    pub(crate) declared_name: Option<String>,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) body: Arc<InterfaceBody>,
}

impl InterfaceInfo {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        name: String,
        file_name: String,
        name_span: Option<TextSpan>,
        type_parameters: Vec<ParsedTypeParameter>,
        extends: Vec<ParsedNamedType>,
        members: Vec<ParsedInterfaceMember>,
        string_index_type: Option<ParsedType>,
        call_signature: Option<ParsedFunctionType>,
        construct_signatures: Vec<ParsedFunctionType>,
        resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ) -> Self {
        Self {
            name,
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
            }),
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
    for member in &incoming.body.members {
        if !is_method(member) && existing_property_names.contains(member.name.as_str()) {
            continue;
        }
        members.push(member.clone());
    }
    let mut extends = existing.body.extends.clone();
    extends.extend(incoming.body.extends.iter().cloned());
    let type_parameters = if existing.body.type_parameters.is_empty() {
        incoming.body.type_parameters.clone()
    } else {
        existing.body.type_parameters.clone()
    };
    InterfaceInfo::new(
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
    )
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

/// Merge every declaration of `source` into `dest`, following the same
/// declaration-merging rules as [`merge_type_declaration_into_table`] but moving
/// payloads by arena pointer instead of deep-cloning them.
///
/// `dest` and `source` must share one [`CheckerArena`] (see
/// [`TypeDeclarationTable::with_arena`]): a fresh insert then becomes a pointer
/// copy of the payload `source` already allocated, and an interface merge
/// allocates the merged result back into the same arena. This is the bulk merge
/// path for ambient/global declaration files, where every lib and `@types`
/// declaration would otherwise be deep-cloned into the global table.
pub(crate) fn merge_shared_arena_table_into(
    dest: &mut TypeDeclarationTable,
    source: &TypeDeclarationTable,
) {
    debug_assert!(
        dest.arena.ptr_eq(&source.arena),
        "shared-arena merge requires dest and source to share one arena"
    );

    enum Action {
        Merge(Box<InterfaceInfo>),
        KeepFirst,
        CopyPayload(usize),
    }

    for (name, id) in source.declarations.iter() {
        let name_ref = name.as_ref();
        let incoming = source.get_by_id(*id);
        let action = match (dest.get(name_ref), incoming) {
            (
                Some(TypeDeclarationInfo::Interface(existing)),
                TypeDeclarationInfo::Interface(incoming),
            ) => Action::Merge(Box::new(merge_interface_infos(existing, incoming))),
            (Some(_), _) => Action::KeepFirst,
            (None, _) => Action::CopyPayload(source.payload_ptr(*id)),
        };

        match action {
            Action::Merge(merged) => dest.upsert(name_ref, TypeDeclarationInfo::Interface(*merged)),
            Action::KeepFirst => {}
            Action::CopyPayload(payload_ptr) => dest.push_shared_payload(name_ref, payload_ptr),
        }
    }
}

/// Bulk variant of [`merge_shared_arena_table_into`] that merges many sources into
/// `dest` in one pass. Applying the single-source merge once per source rebuilds a
/// growing interface's member list on every merge, so a global interface split
/// across N files (`declare global { interface X { ... } }`) was O(N^2). Here each
/// interface is folded into one owned accumulator whose property set is maintained
/// incrementally, making the whole merge O(total members). The observable result
/// is identical: same first-property-wins merge order, same first-wins for
/// cross-kind names. Every source must share `dest`'s arena.
pub(crate) fn merge_shared_arena_tables_into(
    dest: &mut TypeDeclarationTable,
    sources: &[TypeDeclarationTable],
) {
    let is_method = |member: &ParsedInterfaceMember| matches!(member.ty, ParsedType::Function(_));
    // name -> (accumulated interface, its non-method property names)
    let mut interfaces: HashMap<String, (InterfaceInfo, std::collections::HashSet<String>)> =
        HashMap::new();

    for source in sources {
        debug_assert!(
            dest.arena.ptr_eq(&source.arena),
            "shared-arena merge requires dest and source to share one arena"
        );

        for (name, id) in source.declarations.iter() {
            let name_ref = name.as_ref();
            let incoming = source.get_by_id(*id);

            let TypeDeclarationInfo::Interface(incoming_interface) = incoming else {
                // A later non-interface never overrides an earlier binding (first
                // wins), and an interface already accumulated for this name keeps it.
                if dest.get(name_ref).is_some() || interfaces.contains_key(name_ref) {
                    continue;
                }
                dest.push_shared_payload(name_ref, source.payload_ptr(*id));
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
    let body = Arc::make_mut(&mut accumulator.body);
    for member in &incoming.body.members {
        if !is_method(member) {
            if seen.contains(member.name.as_str()) {
                continue;
            }
            seen.insert(member.name.clone());
        }
        body.members.push(member.clone());
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
    if accumulator.resolution_scope.is_none() {
        accumulator.resolution_scope = incoming.resolution_scope.clone();
    }
}

/// A borrowed, context-independent view of an arena-allocated declaration
/// payload.
///
/// The payload lives in a [`CheckerArena`] — an append-only bump allocator
/// behind an `Arc` — so its address is stable for the lifetime of that arena and
/// is never moved by later allocations. Holding a clone of the arena handle keeps
/// the backing memory alive for as long as the handle exists, which lets
/// resolution own a stable `&TypeDeclarationInfo` while the `CheckerContext` the
/// lookup came from is borrowed mutably, without deep-cloning the (often large)
/// interface/alias payload.
#[derive(Clone)]
pub(crate) struct TypeDeclarationHandle {
    _arena: CheckerArena,
    ptr: *const TypeDeclarationInfo,
}

// The pointer is into append-only arena memory kept alive by `_arena`; it is
// only ever read, never written, after the payload is inserted.
unsafe impl Send for TypeDeclarationHandle {}
unsafe impl Sync for TypeDeclarationHandle {}

impl TypeDeclarationHandle {
    pub(crate) fn get(&self) -> &TypeDeclarationInfo {
        unsafe { &*self.ptr }
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
}

#[derive(Debug, Clone, Default)]
/// Shared top-level type-declaration namespace for aliases and interfaces.
///
/// The first declaration wins; later duplicates are reported by the caller and
/// must not replace the original entry.
pub(crate) struct TypeDeclarationTable {
    arena: CheckerArena,
    declarations: HashMap<ArenaStr, TypeDeclarationId>,
    payloads: Vec<usize>,
}

impl TypeDeclarationTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Create an empty table backed by an existing arena. Payloads collected into
    /// this table can then be moved into another table sharing the same arena by
    /// pointer (see [`merge_shared_arena_table_into`]) without a deep clone.
    pub(crate) fn with_arena(arena: CheckerArena) -> Self {
        Self {
            arena,
            declarations: HashMap::new(),
            payloads: Vec::new(),
        }
    }

    pub(crate) fn arena_handle(&self) -> CheckerArena {
        self.arena.clone()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        crate::program::record_declaration_lookup(1);
        self.get_without_lookup_record(name)
    }

    fn get_without_lookup_record(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        let id = self.declarations.get(name)?;
        Some(self.get_by_id(*id))
    }

    fn get_by_id(&self, id: TypeDeclarationId) -> &TypeDeclarationInfo {
        let index = id.0 as usize;
        let ptr = self
            .payloads
            .get(index)
            .expect("type declaration id must point to a stored payload");
        unsafe { &*(*ptr as *const TypeDeclarationInfo) }
    }

    /// Returns a context-independent handle to the payload for `name`, keeping
    /// the backing arena alive without deep-cloning the declaration. See
    /// [`TypeDeclarationHandle`].
    pub(crate) fn get_handle(&self, name: &str) -> Option<TypeDeclarationHandle> {
        let id = *self.declarations.get(name)?;
        let index = id.0 as usize;
        let ptr = *self
            .payloads
            .get(index)
            .expect("type declaration id must point to a stored payload")
            as *const TypeDeclarationInfo;
        Some(TypeDeclarationHandle {
            _arena: self.arena.clone(),
            ptr,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.declarations.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&ArenaStr, &TypeDeclarationInfo)> + '_ {
        self.declarations
            .iter()
            .map(move |(name, id)| (name, self.get_by_id(*id)))
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

        let declaration_id = self.alloc_declaration_payload(declaration);
        let key = ArenaStr::new(name_ref, &self.arena);
        self.declarations.insert(key, declaration_id);
        None
    }

    /// Insert `declaration`, replacing any existing entry for `name`.
    ///
    /// Unlike [`insert`](Self::insert) (which is first-wins), this overwrites the
    /// id mapping with a freshly allocated payload. The previous payload remains
    /// in the append-only arena but is no longer referenced. Used by the default
    /// library declaration-merging path, where later interface declarations must
    /// contribute their members rather than being dropped.
    pub(crate) fn upsert(&mut self, name: impl AsRef<str>, declaration: TypeDeclarationInfo) {
        let name_ref = name.as_ref();
        let declaration_id = self.alloc_declaration_payload(declaration);
        if let Some(existing) = self.declarations.get_mut(name_ref) {
            *existing = declaration_id;
        } else {
            let key = ArenaStr::new(name_ref, &self.arena);
            self.declarations.insert(key, declaration_id);
        }
    }

    fn payload_ptr(&self, id: TypeDeclarationId) -> usize {
        *self
            .payloads
            .get(id.0 as usize)
            .expect("type declaration id must point to a stored payload")
    }

    /// Map `name` to a payload pointer that already lives in this table's arena,
    /// first-wins. The pointer must originate from the same arena as `self`;
    /// callers guarantee this via [`merge_shared_arena_table_into`].
    fn push_shared_payload(&mut self, name: &str, payload_ptr: usize) {
        if self.declarations.contains_key(name) {
            return;
        }
        let id = TypeDeclarationId(self.payloads.len() as u32);
        self.payloads.push(payload_ptr);
        let key = ArenaStr::new(name, &self.arena);
        self.declarations.insert(key, id);
    }

    fn alloc_declaration_payload(&mut self, declaration: TypeDeclarationInfo) -> TypeDeclarationId {
        let declaration = self.arena.alloc_type_declaration_payload(declaration);
        let id = TypeDeclarationId(self.payloads.len() as u32);
        self.payloads
            .push(declaration as *const TypeDeclarationInfo as usize);
        id
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
