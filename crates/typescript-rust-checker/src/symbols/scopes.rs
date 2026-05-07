use crate::symbols::{SymbolInfo, SymbolTable};

#[derive(Debug, Clone)]
pub(crate) struct ScopeStack {
    frames: Vec<ScopeFrame>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeFrame {
    symbols: SymbolTable,
}

impl ScopeStack {
    pub(crate) fn from_root(root: SymbolTable) -> Self {
        Self {
            frames: vec![ScopeFrame { symbols: root }],
        }
    }

    #[allow(dead_code)]
    // Reserved for upcoming scope-aware reads when the checker starts using
    // nested scope lookups outside the current function-body flow path.
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

    pub(crate) fn update_visible(&mut self, name: &str, symbol: SymbolInfo) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame.symbols.get(name).is_some() {
                frame.symbols.insert(name.to_string(), symbol);
                return true;
            }
        }

        false
    }

    pub(crate) fn current_contains_let_or_const(&self, name: &str) -> bool {
        self.frames
            .last()
            .is_some_and(|frame| frame.symbols.contains_let_or_const(name))
    }

    pub(crate) fn push_child(&mut self) {
        self.frames.push(ScopeFrame::default());
    }

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
