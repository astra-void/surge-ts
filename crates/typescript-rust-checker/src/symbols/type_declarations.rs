use std::collections::HashMap;
use std::sync::Arc;

use typescript_rust_syntax::{
    ParsedInterfaceMember, ParsedNamedType, ParsedType, ParsedTypeParameter, TextSpan,
};

#[derive(Debug, Clone)]
pub(crate) struct TypeAliasInfo {
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) ty: ParsedType,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationTable>>,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceInfo {
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) name_span: Option<TextSpan>,
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) extends: Vec<ParsedNamedType>,
    pub(crate) members: Vec<ParsedInterfaceMember>,
    pub(crate) resolution_scope: Option<Arc<TypeDeclarationTable>>,
}

#[derive(Debug, Clone)]
pub(crate) enum TypeDeclarationInfo {
    Alias(TypeAliasInfo),
    Interface(InterfaceInfo),
}

#[derive(Debug, Clone, Default)]
/// Shared top-level type-declaration namespace for aliases and interfaces.
///
/// The first declaration wins; later duplicates are reported by the caller and
/// must not replace the original entry.
pub(crate) struct TypeDeclarationTable {
    declarations: HashMap<String, TypeDeclarationInfo>,
}

impl TypeDeclarationTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        self.declarations.get(name)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &TypeDeclarationInfo)> {
        self.declarations.iter()
    }

    pub(crate) fn insert(
        &mut self,
        name: impl Into<String>,
        declaration: TypeDeclarationInfo,
    ) -> Option<TypeDeclarationInfo> {
        let name = name.into();
        if self.declarations.contains_key(&name) {
            return Some(declaration);
        }

        self.declarations.insert(name, declaration);
        None
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
}
