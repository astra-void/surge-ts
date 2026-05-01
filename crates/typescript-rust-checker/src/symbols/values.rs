use std::collections::HashMap;

use typescript_rust_types::Type;

#[derive(Debug, Clone)]
pub(crate) struct SymbolInfo {
    pub(crate) ty: Type,
    pub(crate) kind: SymbolKind,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolTable {
    symbols: HashMap<String, SymbolInfo>,
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

    pub(crate) fn get(&self, name: &str) -> Option<&SymbolInfo> {
        self.symbols.get(name)
    }

    pub(crate) fn insert(
        &mut self,
        name: impl Into<String>,
        symbol: SymbolInfo,
    ) -> Option<SymbolInfo> {
        self.symbols.insert(name.into(), symbol)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &SymbolInfo)> {
        self.symbols.iter()
    }

    pub(crate) fn contains_let_or_const(&self, name: &str) -> bool {
        self.symbols
            .get(name)
            .is_some_and(|existing| matches!(existing.kind, SymbolKind::Let | SymbolKind::Const))
    }
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
