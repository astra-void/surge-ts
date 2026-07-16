use std::collections::{HashMap, HashSet};

use std::sync::Arc;
use surge_ts_types::fx::FxBuildHasher;

use surge_ts_syntax::{ParsedType, ParsedTypeParameter, TextSpan};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

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
    /// File the signature was declared in. Instantiation re-resolves the parsed
    /// parameter/return annotations, whose names (an imported generic's
    /// module-local types, e.g. react-hook-form's `UseFormProps`) are visible in
    /// the declaring file's per-file scope but not the caller's; the
    /// `module_scope_by_file` fallback keys on the active file name, so
    /// instantiation runs under this one.
    pub(crate) declaring_file: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct SymbolTable {
    // Copy-on-write: clones share this map through `Arc` and only pay for a
    // deep copy if a *shared* table is later mutated. The multi-pass
    // module-binding fixpoint clones symbol tables thousands of times but
    // mutates almost none of those clones, so sharing makes the clones
    // effectively free and the deep copy is deferred to the rare mutate path.
    symbols: Arc<HashMap<Arc<str>, SymbolInfoHandle, FxBuildHasher>>,
    // Name spans of the *first* block-scoped declaration (let/const or function
    // implementation) registered in this scope, so a later redeclaration can
    // back-emit the duplicate diagnostic (TS2451/TS2393) at the original site
    // too — tsc flags every conflicting declaration, not just the latest. Shares
    // the same copy-on-write discipline as `symbols`; empty in nearly every
    // table, so the extra `Arc` clone is effectively free.
    declaration_spans: Arc<HashMap<Arc<str>, TextSpan, FxBuildHasher>>,
    // Names that already have a body-bearing function *implementation* registered
    // in this scope. A second implementation is the only thing that yields
    // TS2393 ("Duplicate function implementation"); bodyless declarations
    // (overload signatures, ambient `declare function`s) merge as overloads and
    // must not trip it. Shares the copy-on-write discipline of `symbols`.
    function_implementations: Arc<HashSet<Arc<str>, FxBuildHasher>>,
    // Optional read-only fallback consulted by lookups (`get`, `get_handle`,
    // `contains_let_or_const`) when a name is absent from `symbols`. A function
    // body's root scope sets this to the module/ambient environment instead of
    // copying every visible symbol into its own map, so opening a scope inside a
    // file with N module-level symbols stays O(1) rather than O(N) (and the COW
    // on the first local insert clones only the small local map, not the parent).
    // Mutations and `iter*` operate on `symbols` alone; `parent` is only ever set
    // on transient, lookup-only scope roots that are never iterated.
    parent: Option<Arc<SymbolTable>>,
}

impl Clone for SymbolTable {
    fn clone(&self) -> Self {
        record_symbol_table_clone_count();
        // Handle/entry copies are now recorded only when a shared table is
        // actually mutated (see `symbols_mut`), since the clone itself copies
        // nothing.
        Self {
            symbols: Arc::clone(&self.symbols),
            declaration_spans: Arc::clone(&self.declaration_spans),
            function_implementations: Arc::clone(&self.function_implementations),
            parent: self.parent.clone(),
        }
    }
}

impl SymbolTable {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }

    /// Build a lookup-only scope whose own map is empty and whose misses fall
    /// through to `parent`. See the `parent` field. Used for function-body roots.
    pub(crate) fn with_parent(parent: Arc<SymbolTable>) -> Self {
        Self {
            symbols: Arc::new(HashMap::default()),
            declaration_spans: Arc::new(HashMap::default()),
            function_implementations: Arc::new(HashSet::default()),
            parent: Some(parent),
        }
    }

    /// Return this table (sharing its own map) with `parent` attached as the
    /// lookup fallback. The own entries keep precedence over `parent`.
    pub(crate) fn with_parent_fallback(mut self, parent: Arc<SymbolTable>) -> Self {
        self.parent = Some(parent);
        self
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
    fn symbols_mut(&mut self) -> &mut HashMap<Arc<str>, SymbolInfoHandle, FxBuildHasher> {
        if Arc::strong_count(&self.symbols) > 1 {
            let entry_count = self.symbols.len() as u64;
            record_symbol_table_entry_handle_copy_count(entry_count);
            record_symbol_info_handle_copy_count(entry_count);
        }
        Arc::make_mut(&mut self.symbols)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&SymbolInfo> {
        record_type_name_lookup_string_count(1);
        match self.symbols.get(name) {
            Some(symbol) => Some(symbol.as_ref()),
            None => self.parent.as_ref().and_then(|parent| parent.get(name)),
        }
    }

    pub(crate) fn get_handle(&self, name: &str) -> Option<SymbolInfoHandle> {
        record_type_name_lookup_string_count(1);
        if let Some(symbol) = self.symbols.get(name) {
            record_symbol_info_handle_copy_count(1);
            return Some(Arc::clone(symbol));
        }
        self.parent
            .as_ref()
            .and_then(|parent| parent.get_handle(name))
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
        if let Some(existing) = self.symbols.get(name) {
            return matches!(existing.as_ref().kind, SymbolKind::Let | SymbolKind::Const);
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.contains_let_or_const(name))
    }

    /// Records the name span of the first declaration of `name` in this scope so
    /// a later redeclaration can back-emit its duplicate diagnostic at the
    /// original site. Only the first recording is kept.
    pub(crate) fn record_declaration_span(&mut self, name: &str, span: TextSpan) {
        if self.declaration_spans.contains_key(name) {
            return;
        }
        Arc::make_mut(&mut self.declaration_spans).insert(name.into(), span);
    }

    /// Removes and returns the recorded first-declaration span for `name`, if
    /// any. Removing ensures a third+ redeclaration does not re-emit at a site
    /// already flagged.
    pub(crate) fn take_declaration_span(&mut self, name: &str) -> Option<TextSpan> {
        if !self.declaration_spans.contains_key(name) {
            return None;
        }
        Arc::make_mut(&mut self.declaration_spans).remove(name)
    }

    /// Whether a body-bearing function implementation for `name` was already
    /// registered in this scope (see [`Self::mark_function_implementation`]).
    pub(crate) fn has_function_implementation(&self, name: &str) -> bool {
        self.function_implementations.contains(name)
    }

    /// Marks that a body-bearing function implementation for `name` exists in
    /// this scope, so a *second* implementation can be reported as TS2393.
    pub(crate) fn mark_function_implementation(&mut self, name: &str) {
        if self.function_implementations.contains(name) {
            return;
        }
        Arc::make_mut(&mut self.function_implementations).insert(name.into());
    }
}

pub(crate) fn clone_symbol_info_handle(symbol: &SymbolInfoHandle) -> SymbolInfoHandle {
    record_symbol_info_handle_copy_count(1);
    Arc::clone(symbol)
}

pub(crate) fn map_symbol_kind(parsed_kind: surge_ts_syntax::ParsedVariableKind) -> SymbolKind {
    match parsed_kind {
        surge_ts_syntax::ParsedVariableKind::Var => SymbolKind::Var,
        surge_ts_syntax::ParsedVariableKind::Let => SymbolKind::Let,
        surge_ts_syntax::ParsedVariableKind::Const => SymbolKind::Const,
    }
}
