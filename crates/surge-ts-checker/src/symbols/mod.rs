mod scopes;
mod type_declarations;
mod values;

pub(crate) use scopes::ScopeStack;
pub(crate) use type_declarations::{
    InterfaceBody, InterfaceInfo, TypeAliasInfo, TypeDeclarationHandle, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable, merge_augmentation_type_declaration_into_table, merge_interface_infos,
    merge_shared_table_into, merge_shared_tables_into, merge_type_declaration_into_table,
};
pub(crate) use values::{
    DestructureKey, TupleDestructureBinding,
    FunctionSignatureInfo, SymbolInfo, SymbolInfoHandle, SymbolKind, SymbolTable,
    clone_symbol_info_handle, map_symbol_kind,
};
