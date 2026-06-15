use std::collections::HashMap;
use std::sync::Arc;

use typescript_rust_syntax::{
    ParsedInterfaceMember, ParsedNamedType, ParsedType, ParsedTypeParameter, TextSpan,
};

use crate::arena::{ArenaStr, CheckerArena};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TypeDeclarationId(u32);

#[derive(Debug)]
pub(crate) struct TypeAliasInfo {
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) ty: ParsedType,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
}

impl Clone for TypeAliasInfo {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_payload_deep_clone_count();
        Self {
            name: self.name.clone(),
            file_name: self.file_name.clone(),
            name_span: self.name_span,
            type_parameters: self.type_parameters.clone(),
            ty: self.ty.clone(),
            resolution_scope: self.resolution_scope.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct InterfaceInfo {
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) extends: Vec<ParsedNamedType>,
    pub(crate) members: Vec<ParsedInterfaceMember>,
    pub(crate) string_index_type: Option<ParsedType>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationScope>>,
}

impl Clone for InterfaceInfo {
    fn clone(&self) -> Self {
        crate::program::record_type_declaration_payload_deep_clone_count();
        Self {
            name: self.name.clone(),
            file_name: self.file_name.clone(),
            name_span: self.name_span,
            type_parameters: self.type_parameters.clone(),
            extends: self.extends.clone(),
            members: self.members.clone(),
            string_index_type: self.string_index_type.clone(),
            resolution_scope: self.resolution_scope.clone(),
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

        let alias = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: "User".to_string(),
            file_name: "a.ts".to_string(),
            name_span: None,
            type_parameters: vec![],
            ty: ParsedType::String,
            resolution_scope: None,
        });
        let interface = TypeDeclarationInfo::Interface(InterfaceInfo {
            name: "User".to_string(),
            file_name: "b.ts".to_string(),
            name_span: None,
            type_parameters: vec![],
            extends: vec![],
            members: vec![],
            string_index_type: None,
            resolution_scope: None,
        });

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
        let ambient_alias = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: "User".to_string(),
            file_name: "ambient.d.ts".to_string(),
            name_span: None,
            type_parameters: vec![],
            ty: ParsedType::String,
            resolution_scope: None,
        });
        let _ = ambient.insert("User", ambient_alias.clone());

        let mut local = TypeDeclarationTable::new();
        let local_interface = TypeDeclarationInfo::Interface(InterfaceInfo {
            name: "User".to_string(),
            file_name: "local.ts".to_string(),
            name_span: None,
            type_parameters: vec![],
            extends: vec![],
            members: vec![],
            string_index_type: None,
            resolution_scope: None,
        });
        let _ = local.insert("User", local_interface.clone());

        let scope = TypeDeclarationScope::new(vec![Arc::new(local), Arc::new(ambient)]);

        assert_eq!(scope.layer_count(), 2);
        assert!(matches!(
            scope.get("User"),
            Some(TypeDeclarationInfo::Interface(_))
        ));

        let mut ambient_only = TypeDeclarationTable::new();
        let ambient_only_alias = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: "Buffer".to_string(),
            file_name: "ambient.d.ts".to_string(),
            name_span: None,
            type_parameters: vec![],
            ty: ParsedType::Unknown,
            resolution_scope: None,
        });
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
