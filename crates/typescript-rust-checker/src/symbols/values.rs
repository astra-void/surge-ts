use std::collections::HashMap;
use std::sync::Arc;

use typescript_rust_syntax::{ParsedType, ParsedTypeParameter};
use typescript_rust_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::program::{
    record_symbol_info_handle_copy_count, record_symbol_info_payload_deep_clone_count,
    record_symbol_table_clone_count, record_symbol_table_entry_handle_copy_count,
    record_type_name_lookup_string_count,
};

pub(crate) type SymbolInfoHandle = Arc<SymbolInfo>;

#[derive(Debug)]
pub(crate) struct SymbolInfo {
    pub(crate) ty: Type,
    pub(crate) kind: SymbolKind,
    pub(crate) function_signature: Option<FunctionSignatureInfo>,
}

impl Clone for SymbolInfo {
    fn clone(&self) -> Self {
        record_symbol_info_payload_deep_clone_count();
        Self {
            ty: self.ty.clone(),
            kind: self.kind,
            function_signature: self.function_signature.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionSignatureInfo {
    pub(crate) type_parameters: Vec<ParsedTypeParameter>,
    pub(crate) parameter_types: Vec<Option<ParsedType>>,
    pub(crate) return_type: Option<ParsedType>,
}

#[derive(Debug, Default)]
pub(crate) struct SymbolTable {
    // Copy-on-write: clones share this map through `Arc` and only pay for a
    // deep copy if a *shared* table is later mutated. The multi-pass
    // module-binding fixpoint clones symbol tables thousands of times but
    // mutates almost none of those clones, so sharing makes the clones
    // effectively free and the deep copy is deferred to the rare mutate path.
    symbols: Arc<HashMap<Arc<str>, SymbolInfoHandle>>,
}

impl Clone for SymbolTable {
    fn clone(&self) -> Self {
        record_symbol_table_clone_count();
        // Handle/entry copies are now recorded only when a shared table is
        // actually mutated (see `symbols_mut`), since the clone itself copies
        // nothing.
        Self {
            symbols: Arc::clone(&self.symbols),
        }
    }
}

impl SymbolTable {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    Var,
    Let,
    Const,
    Function,
    Parameter,
}

impl SymbolTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Returns a uniquely-owned mutable view of the underlying map, performing
    /// the copy-on-write deep copy only if this table currently shares its map
    /// with a clone. The deep copy is what the per-entry handle-copy counters
    /// now measure.
    fn symbols_mut(&mut self) -> &mut HashMap<Arc<str>, SymbolInfoHandle> {
        if Arc::strong_count(&self.symbols) > 1 {
            let entry_count = self.symbols.len() as u64;
            record_symbol_table_entry_handle_copy_count(entry_count);
            record_symbol_info_handle_copy_count(entry_count);
        }
        Arc::make_mut(&mut self.symbols)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&SymbolInfo> {
        record_type_name_lookup_string_count(1);
        self.symbols.get(name).map(Arc::as_ref)
    }

    pub(crate) fn get_handle(&self, name: &str) -> Option<SymbolInfoHandle> {
        record_type_name_lookup_string_count(1);
        self.symbols.get(name).map(|symbol| {
            record_symbol_info_handle_copy_count(1);
            Arc::clone(symbol)
        })
    }

    pub(crate) fn get_shared(&self, name: &str) -> Option<SymbolInfoHandle> {
        self.get_handle(name)
    }

    pub(crate) fn insert(
        &mut self,
        name: impl Into<Arc<str>>,
        symbol: SymbolInfo,
    ) -> Option<SymbolInfoHandle> {
        self.symbols_mut().insert(name.into(), Arc::new(symbol))
    }

    pub(crate) fn insert_handle(
        &mut self,
        name: impl Into<Arc<str>>,
        symbol: SymbolInfoHandle,
    ) -> Option<SymbolInfoHandle> {
        self.symbols_mut().insert(name.into(), symbol)
    }

    pub(crate) fn insert_shared(
        &mut self,
        name: impl Into<Arc<str>>,
        symbol: SymbolInfoHandle,
    ) -> Option<SymbolInfoHandle> {
        self.insert_handle(name, symbol)
    }

    pub(crate) fn remove(&mut self, name: &str) -> Option<SymbolInfoHandle> {
        self.symbols_mut().remove(name)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &SymbolInfo)> {
        self.symbols
            .iter()
            .map(|(name, symbol)| (name, symbol.as_ref()))
    }

    pub(crate) fn iter_handles(&self) -> impl Iterator<Item = (&Arc<str>, &SymbolInfoHandle)> {
        self.symbols.iter()
    }

    pub(crate) fn iter_shared(&self) -> impl Iterator<Item = (&Arc<str>, &SymbolInfoHandle)> {
        self.iter_handles()
    }

    pub(crate) fn contains_let_or_const(&self, name: &str) -> bool {
        record_type_name_lookup_string_count(1);
        self.symbols.get(name).is_some_and(|existing| {
            matches!(existing.as_ref().kind, SymbolKind::Let | SymbolKind::Const)
        })
    }
}

pub(crate) fn clone_symbol_info_handle(symbol: &SymbolInfoHandle) -> SymbolInfoHandle {
    record_symbol_info_handle_copy_count(1);
    Arc::clone(symbol)
}

pub(crate) fn map_symbol_kind(
    parsed_kind: typescript_rust_syntax::ParsedVariableKind,
) -> SymbolKind {
    match parsed_kind {
        typescript_rust_syntax::ParsedVariableKind::Var => SymbolKind::Var,
        typescript_rust_syntax::ParsedVariableKind::Let => SymbolKind::Let,
        typescript_rust_syntax::ParsedVariableKind::Const => SymbolKind::Const,
    }
}
