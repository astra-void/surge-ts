mod scopes;
mod type_declarations;
mod values;

pub(crate) use scopes::ScopeStack;
pub(crate) use type_declarations::{
    InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationTable,
};
pub(crate) use values::{SymbolInfo, SymbolKind, SymbolTable, map_symbol_kind};
