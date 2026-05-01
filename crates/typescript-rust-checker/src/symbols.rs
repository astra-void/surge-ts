use std::collections::HashMap;

use typescript_rust_syntax::ParsedVariableKind;
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

#[derive(Debug, Clone)]
pub(crate) struct ScopeStack {
    frames: Vec<ScopeFrame>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeFrame {
    symbols: SymbolTable,
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

impl ScopeStack {
    pub(crate) fn from_root(root: SymbolTable) -> Self {
        Self {
            frames: vec![ScopeFrame { symbols: root }],
        }
    }

    #[allow(dead_code)]
    pub(crate) fn resolve(&self, name: &str) -> Option<&SymbolInfo> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.symbols.get(name))
    }

    pub(crate) fn insert_current(
        &mut self,
        name: impl Into<String>,
        symbol: SymbolInfo,
    ) -> Option<SymbolInfo> {
        self.frames
            .last_mut()
            .expect("scope stack must contain at least one frame")
            .symbols
            .insert(name, symbol)
    }

    pub(crate) fn current_contains_let_or_const(&self, name: &str) -> bool {
        self.frames
            .last()
            .is_some_and(|frame| frame.symbols.contains_let_or_const(name))
    }

    pub(crate) fn push_child(&mut self) {
        self.frames.push(ScopeFrame::default());
    }

    #[allow(dead_code)]
    pub(crate) fn pop_child(&mut self) {
        assert!(
            !self.frames.is_empty(),
            "scope stack must contain at least one frame"
        );
        self.frames.pop();
    }

    pub(crate) fn visible_symbols(&self) -> SymbolTable {
        let mut visible = SymbolTable::new();

        for frame in &self.frames {
            for (name, symbol) in frame.symbols.iter() {
                visible.insert(name.clone(), symbol.clone());
            }
        }

        visible
    }
}

pub(crate) fn map_symbol_kind(parsed_kind: ParsedVariableKind) -> SymbolKind {
    match parsed_kind {
        ParsedVariableKind::Var => SymbolKind::Var,
        ParsedVariableKind::Let => SymbolKind::Let,
        ParsedVariableKind::Const => SymbolKind::Const,
    }
}
