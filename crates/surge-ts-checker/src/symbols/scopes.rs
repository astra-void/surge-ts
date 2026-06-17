use std::collections::HashMap;
use std::sync::Arc;

use crate::program::record_scope_stack_visible_symbol_handle_copy_count;
use crate::symbols::{SymbolInfo, SymbolInfoHandle, SymbolTable, clone_symbol_info_handle};

#[derive(Debug, Clone)]
pub(crate) struct ScopeStack {
    frames: Vec<ScopeFrame>,
    visible_symbols: SymbolTable,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeFrame {
    symbols: SymbolTable,
    visible_shadows: HashMap<Arc<str>, Option<SymbolInfoHandle>>,
}

impl ScopeStack {
    pub(crate) fn from_root(root: SymbolTable) -> Self {
        // Share the root (module/ambient) symbols through `parent` fallback rather
        // than as the own map of either table. Both `visible_symbols` and the root
        // frame are mutated on every local binding; holding `root` as their own
        // map made the first insert copy the entire module symbol table per
        // function body (the copy-on-write deep copy fired because the Arc was
        // shared). With the fallback, inserts hit small unshared own maps and
        // lookups still fall through to the full root. The fallback table is only
        // ever read via `get`, never iterated.
        let root = Arc::new(root);
        Self {
            visible_symbols: SymbolTable::with_parent(root.clone()),
            frames: vec![ScopeFrame {
                symbols: SymbolTable::with_parent(root),
                visible_shadows: HashMap::new(),
            }],
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
        name: impl Into<Arc<str>>,
        symbol: SymbolInfo,
    ) -> Option<SymbolInfoHandle> {
        self.insert_current_handle(name, Arc::new(symbol))
    }

    pub(crate) fn insert_current_handle(
        &mut self,
        name: impl Into<Arc<str>>,
        symbol: SymbolInfoHandle,
    ) -> Option<SymbolInfoHandle> {
        let name = name.into();
        let previous_visible = self.visible_symbols.get_handle(&name);
        let current_frame = self
            .frames
            .last_mut()
            .expect("scope stack must contain at least one frame");
        if current_frame.symbols.get(&name).is_none() {
            current_frame
                .visible_shadows
                .insert(name.clone(), previous_visible);
        }

        record_scope_stack_visible_symbol_handle_copy_count(1);
        self.visible_symbols
            .insert_handle(name.clone(), clone_symbol_info_handle(&symbol));
        current_frame.symbols.insert_handle(name, symbol)
    }

    pub(crate) fn update_visible(&mut self, name: &str, symbol: SymbolInfo) -> bool {
        self.update_visible_handle(name, Arc::new(symbol))
    }

    pub(crate) fn update_visible_handle(&mut self, name: &str, symbol: SymbolInfoHandle) -> bool {
        let name: Arc<str> = name.into();
        for frame in self.frames.iter_mut().rev() {
            if frame.symbols.get(&name).is_some() {
                record_scope_stack_visible_symbol_handle_copy_count(1);
                self.visible_symbols
                    .insert_handle(name.clone(), clone_symbol_info_handle(&symbol));
                frame.symbols.insert_handle(name, symbol);
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
        let frame = self.frames.pop().expect("scope stack must contain a frame");
        for (name, previous_symbol) in frame.visible_shadows {
            match previous_symbol {
                Some(previous_symbol) => {
                    record_scope_stack_visible_symbol_handle_copy_count(1);
                    self.visible_symbols.insert_handle(name, previous_symbol);
                }
                None => {
                    self.visible_symbols.remove(&name);
                }
            }
        }
    }

    pub(crate) fn visible_symbols(&self) -> &SymbolTable {
        &self.visible_symbols
    }
}
