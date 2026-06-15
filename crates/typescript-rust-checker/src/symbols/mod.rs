mod scopes;
mod type_declarations;
mod values;

pub(crate) use scopes::ScopeStack;
pub(crate) use type_declarations::{
    InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope, TypeDeclarationTable,
    merge_interface_infos, merge_type_declaration_into_table,
};
pub(crate) use values::{
    FunctionSignatureInfo, SymbolInfo, SymbolInfoHandle, SymbolKind, SymbolTable,
    clone_symbol_info_handle, map_symbol_kind,
};
