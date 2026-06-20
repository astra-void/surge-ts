use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use surge_ts_diagnostics::{Diagnostic, TextSpan as DiagnosticTextSpan};
use surge_ts_syntax::{ParsedType, ParsedTypeParameter, TextSpan as SyntaxTextSpan};
use surge_ts_types::Type;

use crate::program::ProgramTimings;
use crate::symbols::{
    SymbolTable, TypeDeclarationInfo, TypeDeclarationScope, TypeDeclarationTable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticProfile {
    #[default]
    Tsc,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    RootSource,
    RootDeclaration,
    DependencyDeclaration,
    GeneratedDeclaration,
    /// A physical TypeScript `lib*.d.ts` default-lib file loaded from the
    /// installed `typescript` package (the default in project mode). Lowered
    /// through the real ambient-global pipeline, but its own diagnostics are
    /// suppressed like any other trusted upstream library file.
    PhysicalDefaultLib,
}

impl FileKind {
    pub(crate) fn is_declaration(self) -> bool {
        matches!(
            self,
            FileKind::RootDeclaration
                | FileKind::DependencyDeclaration
                | FileKind::GeneratedDeclaration
                | FileKind::PhysicalDefaultLib
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompatibilityStats {
    pub suppressed_diagnostics_total: usize,
    pub suppressed_declaration_diagnostics_total: usize,
    pub suppressed_rust_only_diagnostics_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
    pub no_implicit_returns: bool,
    pub no_fallthrough_cases_in_switch: bool,
    pub no_implicit_override: bool,
    pub no_property_access_from_index_signature: bool,
    pub stub_external_modules: bool,
    pub resolved_modules: std::collections::HashMap<String, String>,
    /// Effective type-package names included in the program. When the project's
    /// `compilerOptions.types` used the `"*"` wildcard, the literal `"*"` is kept
    /// in this list as a sentinel (see [`Self::types_uses_wildcard`]); it never
    /// matches a real `@types` package path, so the other consumers ignore it.
    pub types: Vec<String>,
    pub no_lib: bool,
    pub skip_lib_check: bool,
    pub diagnostic_profile: DiagnosticProfile,
}

impl CheckerOptions {
    /// Whether `compilerOptions.types` contained the `"*"` wildcard. Selects the
    /// node install-hint variant (TS2580 with a wildcard, TS2591 without),
    /// matching TypeScript's `usesWildcardTypes` branch.
    pub(crate) fn types_uses_wildcard(&self) -> bool {
        self.types.iter().any(|name| name == "*")
    }
}

impl Default for CheckerOptions {
    fn default() -> Self {
        Self {
            no_implicit_any: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            stub_external_modules: false,
            resolved_modules: std::collections::HashMap::new(),
            types: Vec::new(),
            no_lib: false,
            skip_lib_check: false,
            diagnostic_profile: DiagnosticProfile::default(),
        }
    }
}

use crate::modules::ModuleExportTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DeclarationNamespace {
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeclarationResolutionKey {
    pub(crate) file_name: String,
    pub(crate) name: String,
    pub(crate) namespace: DeclarationNamespace,
}

#[derive(Debug, Clone)]
pub(crate) enum DeclarationResolutionState {
    Resolving,
    Resolved { ty: Type, had_error: bool },
}

#[derive(Debug, Clone)]
pub(crate) struct GenericInstantiationCacheEntry {
    pub(crate) arguments: Vec<Type>,
    pub(crate) ty: Type,
    pub(crate) had_error: bool,
}

/// One memoized structural expansion of a named declaration at a fixed set of
/// resolved type arguments, shared via `Arc` so a `Type::Reference` can resolve
/// to it without re-expanding the declaration body. Backs the lazy/nominal type
/// reference machinery (see `infer::types` instantiation interner).
#[derive(Debug, Clone)]
pub(crate) struct InstantiationCacheEntry {
    pub(crate) arguments: Vec<Type>,
    pub(crate) resolved: std::sync::Arc<Type>,
}

#[derive(Debug, Clone)]
pub(crate) struct CheckerContext {
    pub(crate) file_name: String,
    pub(crate) current_file_kind: FileKind,
    pub(crate) options: CheckerOptions,
    pub(crate) diagnostics: Vec<Diagnostic>,
    // Dedup index for `push`, mirroring the keys of `diagnostics`. `push` rejected
    // duplicates by scanning the whole `diagnostics` vec (re-rendering every code
    // to a `String` per comparison), so a context that emits D diagnostics was
    // O(D^2) — e.g. a single file with thousands of unresolved-name reports. The
    // set makes the check O(1); `diagnostic_keys_len` lets `push` detect when
    // `diagnostics` was mutated directly (clear/take/truncate) and rebuild lazily.
    diagnostic_keys: HashSet<(String, String, String, Option<surge_ts_diagnostics::TextSpan>)>,
    diagnostic_keys_len: usize,
    pub(crate) stats: CompatibilityStats,
    pub(crate) utility_diagnostic_keys: HashSet<UtilityDiagnosticKey>,
    pub(crate) symbols: SymbolTable,
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) type_declaration_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) resolved_named_types:
        Arc<Mutex<HashMap<DeclarationResolutionKey, DeclarationResolutionState>>>,
    /// Program-scoped cache for context-free *generic* library/dependency
    /// instantiations, keyed by declaration and the resolved type arguments. The
    /// real `lib*.d.ts` typed-array/iterator cluster (`Uint8Array`,
    /// `ArrayIterator`, `IteratorObject`, …) is mutually recursive and generic, so
    /// every signature mentioning it would otherwise re-expand the entire tree.
    /// Each bucket is a small list of `(resolved args, resolution)` checked by
    /// structural `Type` equality, so a fingerprint collision can never return a
    /// wrong type. Only top-level (`resolving` empty) instantiations are stored, so
    /// the cached value matches a standalone resolution. Never reset; shared via
    /// `Arc` across all `CheckerContext` clones and jobs.
    pub(crate) program_resolved_generic_types:
        Arc<Mutex<HashMap<DeclarationResolutionKey, Vec<GenericInstantiationCacheEntry>>>>,
    /// Program-wide instantiation interner backing lazy/nominal `Type::Reference`
    /// resolution. Maps a declaration + resolved type arguments to the shared
    /// structural expansion, so a reference resolves (and the body expands) at
    /// most once per unique instantiation rather than at every use site. Shared
    /// via `Arc` across all `CheckerContext` clones and jobs.
    pub(crate) program_instantiations:
        Arc<Mutex<HashMap<DeclarationResolutionKey, Vec<InstantiationCacheEntry>>>>,
    pub(crate) ambient_modules: Arc<std::collections::HashMap<String, ModuleExportTable>>,
    /// Module augmentations (`declare module "x"` in a file that is itself a
    /// module). Unlike ambient module declarations, these only merge into an
    /// already-resolved target; they do not make `"x"` resolvable on their own.
    pub(crate) module_augmentations: Arc<std::collections::HashMap<String, ModuleExportTable>>,
    pub(crate) ambient_global_symbols: SymbolTable,
    pub(crate) ambient_global_type_declarations: Arc<TypeDeclarationTable>,
    pub(crate) module_file_index_by_identity: Arc<HashMap<Arc<str>, usize>>,
    /// Each module's resolution scope keyed by its source `file_name`, mirroring
    /// `shared_state.module_resolution_scopes` but addressable by name. A type
    /// alias/interface imported across a module *import cycle* can lose its
    /// pre-attached `resolution_scope` when the multi-pass binding fixpoint rebinds
    /// it (the source module's scope is not yet available in that pass), leaving it
    /// `None`. Resolving such a declaration's body must still happen in its
    /// declaring module's scope, so resolution falls back to this map keyed by the
    /// declaration's `file_name`. Populated once before the check phase; empty
    /// during the binding passes (where no diagnostics surface).
    pub(crate) module_scope_by_file: Arc<HashMap<Arc<str>, Arc<TypeDeclarationScope>>>,
    pub(crate) type_parameter_scopes: Vec<HashMap<String, Type>>,
    // Parallel to `type_parameter_scopes`: the declared constraint (if any) for
    // each in-scope type parameter, used to recognize `K extends keyof T` so a
    // generic `T[K]` is not falsely reported as an invalid index (TS2536).
    pub(crate) type_parameter_constraint_scopes: Vec<HashMap<String, ParsedType>>,
    pub(crate) timings: Option<std::sync::Arc<std::sync::Mutex<ProgramTimings>>>,
    /// Nonzero while resolving the body of a namespace-qualified type member
    /// (e.g. `React.ComponentProps`). Unresolved names encountered here are
    /// internal references into a namespace surface we only partially model, so
    /// they resolve to `unknown` without a TS2304 cascade — tsc resolves them
    /// against the full `@types/*`/generated namespace and reports nothing.
    pub(crate) namespace_member_resolution_depth: usize,
    /// Stack of namespace prefixes for the member bodies currently being
    /// resolved (e.g. `"React"` while expanding `React.ChangeEventHandler`).
    /// Namespace members are stored under qualified names but reference their
    /// siblings unqualified (`EventHandler<…>` inside `React.ChangeEventHandler`),
    /// so a bare name that does not resolve is retried against these prefixes.
    pub(crate) namespace_member_prefix_stack: Vec<String>,
    /// Lowest `resolving`-stack index that any cycle truncation has re-entered
    /// since this field was last reset. A resolution that pushed its declaration
    /// at stack depth `floor` is independent of the enclosing `resolving` context
    /// — and therefore safe to memoize — only if every cycle it triggered
    /// re-entered a frame at `floor` or deeper (an *internal* self/mutual cycle).
    /// A cycle reaching below `floor` means the result depends on an outer frame.
    /// See the generic instantiation cache in `resolve_named_type`.
    pub(crate) lowest_cycle_target_index: usize,
    /// Shared, immutable snapshot of this context used to resolve a lazy library
    /// [`Type::Reference`] body on demand (see the lazy instantiation resolver in
    /// `infer::types`). Captured once, lazily, when the first library reference is
    /// deferred; the `Arc`-shared caches/ambient surface stay live through it, so a
    /// peel memoizes into the same program-wide interner. Cleared of diagnostics —
    /// a lazy peel emits none.
    lazy_resolution_snapshot: Option<Arc<CheckerContext>>,
    file_kinds: Arc<HashMap<String, FileKind>>,
    /// All module-scope value bindings of the file currently being checked,
    /// inferred up front. Consulted only when a bare identifier misses the
    /// positional scope, so a function body may reference a `const`/`let`/`class`
    /// declared *after* it (legal — the body runs after the module finishes).
    /// `Arc`-shared so cloning the context stays cheap.
    pub(crate) module_value_fallback: Option<Arc<SymbolTable>>,
}

impl CheckerContext {
    pub(crate) fn new(
        file_name: String,
        options: CheckerOptions,
        file_kinds: HashMap<String, FileKind>,
    ) -> Self {
        let current_file_kind = file_kinds
            .get(&file_name)
            .copied()
            .unwrap_or(FileKind::RootSource);

        Self {
            file_name,
            current_file_kind,
            options,
            diagnostics: Vec::new(),
            diagnostic_keys: HashSet::new(),
            diagnostic_keys_len: 0,
            stats: CompatibilityStats::default(),
            utility_diagnostic_keys: HashSet::new(),
            symbols: SymbolTable::new(),
            type_declarations: TypeDeclarationTable::new(),
            type_declaration_scope: None,
            resolved_named_types: Arc::new(Mutex::new(HashMap::new())),
            program_resolved_generic_types: Arc::new(Mutex::new(HashMap::new())),
            program_instantiations: Arc::new(Mutex::new(HashMap::new())),
            ambient_modules: Arc::new(std::collections::HashMap::new()),
            module_augmentations: Arc::new(std::collections::HashMap::new()),
            ambient_global_symbols: SymbolTable::new(),
            ambient_global_type_declarations: Arc::new(TypeDeclarationTable::new()),
            module_file_index_by_identity: Arc::new(HashMap::new()),
            module_scope_by_file: Arc::new(HashMap::new()),
            type_parameter_scopes: Vec::new(),
            type_parameter_constraint_scopes: Vec::new(),
            timings: None,
            namespace_member_resolution_depth: 0,
            namespace_member_prefix_stack: Vec::new(),
            lowest_cycle_target_index: usize::MAX,
            lazy_resolution_snapshot: None,
            file_kinds: Arc::new(file_kinds),
            module_value_fallback: None,
        }
    }

    pub(crate) fn note_resolution_cycle(&mut self, target_index: usize) {
        self.lowest_cycle_target_index = self.lowest_cycle_target_index.min(target_index);
    }

    /// Returns the shared snapshot used to resolve a deferred library reference
    /// body, capturing it on first use. The snapshot keeps the `Arc`-shared caches
    /// and ambient surface (so a peel interns into the same program-wide store) but
    /// drops the per-file diagnostic state and its own snapshot back-pointer.
    pub(crate) fn lazy_resolution_snapshot(&mut self) -> Arc<CheckerContext> {
        if let Some(snapshot) = &self.lazy_resolution_snapshot {
            return snapshot.clone();
        }
        let mut snapshot = self.clone();
        snapshot.diagnostics = Vec::new();
        snapshot.diagnostic_keys = HashSet::new();
        snapshot.diagnostic_keys_len = 0;
        snapshot.lazy_resolution_snapshot = None;
        let snapshot = Arc::new(snapshot);
        self.lazy_resolution_snapshot = Some(snapshot.clone());
        snapshot
    }

    /// Whether unresolved type names should be silently treated as `unknown`
    /// rather than emitting TS2304 — true while expanding a namespace-qualified
    /// member body. See [`Self::namespace_member_resolution_depth`].
    pub(crate) fn suppress_unknown_type_name(&self) -> bool {
        self.namespace_member_resolution_depth > 0
    }

    pub(crate) fn push_type_parameter_scope(
        &mut self,
        type_parameters: &[ParsedTypeParameter],
        substitution: Option<HashMap<String, Type>>,
    ) {
        let mut scope = substitution.unwrap_or_default();
        let mut constraint_scope = HashMap::new();
        for type_parameter in type_parameters {
            scope
                .entry(type_parameter.name.clone())
                .or_insert(Type::Unknown);
            if let Some(constraint) = type_parameter.constraint.clone() {
                constraint_scope.insert(type_parameter.name.clone(), constraint);
            }
        }
        self.type_parameter_scopes.push(scope);
        self.type_parameter_constraint_scopes.push(constraint_scope);
    }

    pub(crate) fn pop_type_parameter_scope(&mut self) {
        self.type_parameter_scopes
            .pop()
            .expect("type parameter scope stack must not underflow");
        self.type_parameter_constraint_scopes
            .pop()
            .expect("type parameter constraint scope stack must not underflow");
    }

    /// When the in-scope type parameter `name` is declared as `name extends keyof X`,
    /// return the referenced type-parameter name `X`. Used to keep generic `T[K]`
    /// indexed access valid when `K extends keyof T`.
    pub(crate) fn type_parameter_keyof_constraint_target(&self, name: &str) -> Option<&str> {
        for scope in self.type_parameter_constraint_scopes.iter().rev() {
            if let Some(constraint) = scope.get(name) {
                if let ParsedType::KeyOf(inner) = constraint
                    && let ParsedType::Named(named) = inner.as_ref()
                {
                    return Some(named.name.as_str());
                }
                return None;
            }
        }
        None
    }

    /// Whether the in-scope type parameter `name` was declared with any
    /// `extends` constraint. A constrained parameter's valid index keys depend
    /// on its (often complex, library-generated) constraint, which we do not
    /// fully resolve; tsc validates the access against that constraint, so an
    /// indexed access through a constrained parameter must not cascade into a
    /// `TS2536`/`TS2538` false positive.
    pub(crate) fn type_parameter_has_constraint(&self, name: &str) -> bool {
        self.type_parameter_constraint_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    pub(crate) fn set_file_name(&mut self, file_name: String) {
        self.current_file_kind = self
            .file_kinds
            .get(&file_name)
            .copied()
            .unwrap_or(FileKind::RootSource);
        self.file_name = file_name;
    }

    pub(crate) fn set_symbols(&mut self, symbols: SymbolTable) {
        self.symbols = symbols;
    }

    /// Whether `file_name` is a trusted upstream library/dependency declaration
    /// file. Resolutions of declarations in such files are context-free (their
    /// bodies reference only the global ambient surface) and emit no use-site
    /// diagnostics under `skipLibCheck`, so they are safe to memoize program-wide.
    pub(crate) fn is_library_scoped_file(&self, file_name: &str) -> bool {
        if crate::default_lib::is_physical_default_lib_file_name(file_name)
            || crate::default_lib::is_generated_default_lib_file_name(file_name)
        {
            return true;
        }
        matches!(
            self.file_kinds.get(file_name),
            Some(
                FileKind::DependencyDeclaration
                    | FileKind::GeneratedDeclaration
                    | FileKind::PhysicalDefaultLib
            )
        )
    }

    pub(crate) fn lookup_type_declaration(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        if let Some(declaration) = self.lookup_type_declaration_exact(name) {
            return Some(declaration);
        }
        for candidate in self.namespace_qualified_candidates(name) {
            if let Some(declaration) = self.lookup_type_declaration_exact(&candidate) {
                return Some(declaration);
            }
        }
        None
    }

    fn lookup_type_declaration_exact(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        if let Some(declaration) = self.type_declarations.get(name) {
            crate::program::record_type_declaration_lookup(1);
            return Some(declaration);
        }

        if let Some(scope) = self.type_declaration_scope.as_ref() {
            if let Some(declaration) = scope.get(name) {
                crate::program::record_type_declaration_lookup(2);
                return Some(declaration);
            }
        }

        crate::program::record_type_declaration_lookup(3);
        self.ambient_global_type_declarations.get(name)
    }

    /// Candidate qualified names for a bare reference made inside a namespace
    /// member body: the innermost active prefix and each enclosing one, joined to
    /// `name` (`React.X`; `A.B.X` then `A.X`). Empty unless a namespace member is
    /// being resolved, or when `name` is already qualified.
    fn namespace_qualified_candidates(&self, name: &str) -> Vec<String> {
        if name.contains('.') {
            return Vec::new();
        }
        let Some(prefix) = self.namespace_member_prefix_stack.last() else {
            return Vec::new();
        };

        let mut candidates = Vec::new();
        let mut remaining = prefix.as_str();
        loop {
            candidates.push(format!("{remaining}.{name}"));
            match remaining.rsplit_once('.') {
                Some((outer, _)) => remaining = outer,
                None => break,
            }
        }
        candidates
    }

    /// Like [`lookup_type_declaration`](Self::lookup_type_declaration) but returns
    /// a [`TypeDeclarationHandle`] whose borrow is decoupled from `self`, so
    /// resolution can read the declaration while `self` is borrowed mutably
    /// without deep-cloning the payload.
    pub(crate) fn lookup_type_declaration_handle(
        &self,
        name: &str,
    ) -> Option<crate::symbols::TypeDeclarationHandle> {
        if let Some(handle) = self.lookup_type_declaration_handle_exact(name) {
            return Some(handle);
        }
        for candidate in self.namespace_qualified_candidates(name) {
            if let Some(handle) = self.lookup_type_declaration_handle_exact(&candidate) {
                return Some(handle);
            }
        }
        None
    }

    fn lookup_type_declaration_handle_exact(
        &self,
        name: &str,
    ) -> Option<crate::symbols::TypeDeclarationHandle> {
        if let Some(handle) = self.type_declarations.get_handle(name) {
            crate::program::record_type_declaration_lookup(1);
            return Some(handle);
        }

        if let Some(scope) = self.type_declaration_scope.as_ref() {
            if let Some(handle) = scope.get_handle(name) {
                crate::program::record_type_declaration_lookup(2);
                return Some(handle);
            }
        }

        crate::program::record_type_declaration_lookup(3);
        self.ambient_global_type_declarations.get_handle(name)
    }

    pub(crate) fn set_module_file_index_by_identity(
        &mut self,
        module_file_index_by_identity: HashMap<Arc<str>, usize>,
    ) {
        self.module_file_index_by_identity = Arc::new(module_file_index_by_identity);
    }

    pub(crate) fn set_module_scope_by_file(
        &mut self,
        module_scope_by_file: HashMap<Arc<str>, Arc<TypeDeclarationScope>>,
    ) {
        self.module_scope_by_file = Arc::new(module_scope_by_file);
    }

    /// The resolution scope of the module that declared `file_name`, used as a
    /// fallback when a declaration's pre-attached `resolution_scope` was dropped
    /// across the cyclic-import binding fixpoint. See [`Self::module_scope_by_file`].
    pub(crate) fn module_scope_for_file(
        &self,
        file_name: &str,
    ) -> Option<Arc<TypeDeclarationScope>> {
        self.module_scope_by_file.get(file_name).cloned()
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        if self.should_suppress(&diagnostic) {
            self.record_suppressed(&diagnostic);
            return;
        }

        // `diagnostics` is also mutated directly elsewhere (clear / mem::take /
        // truncate); if its length no longer matches what the index reflects, the
        // index is stale, so rebuild it from the current diagnostics before use.
        if self.diagnostic_keys_len != self.diagnostics.len() {
            self.diagnostic_keys = self
                .diagnostics
                .iter()
                .map(Self::diagnostic_dedup_key)
                .collect();
            self.diagnostic_keys_len = self.diagnostics.len();
        }

        if !self.diagnostic_keys.insert(Self::diagnostic_dedup_key(&diagnostic)) {
            return;
        }

        self.diagnostics.push(diagnostic);
        self.diagnostic_keys_len = self.diagnostics.len();
    }

    fn diagnostic_dedup_key(
        diagnostic: &Diagnostic,
    ) -> (String, String, String, Option<surge_ts_diagnostics::TextSpan>) {
        (
            diagnostic.code.to_string(),
            diagnostic.file_name.clone(),
            diagnostic.message.clone(),
            diagnostic.span,
        )
    }

    pub(crate) fn push_utility_diagnostic_once(&mut self, diagnostic: Diagnostic) {
        let key = UtilityDiagnosticKey {
            code: diagnostic.code.to_string(),
            file_name: diagnostic.file_name.clone(),
            span: diagnostic.span.map(|span| (span.start, span.end)),
            message: diagnostic.message.clone(),
        };

        if self.utility_diagnostic_keys.insert(key) {
            self.push(diagnostic);
        }
    }

    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn truncate_diagnostics(&mut self, len: usize) {
        self.diagnostics.truncate(len);
    }

    /// Like [`truncate_diagnostics`] but also releases the
    /// `push_utility_diagnostic_once` keys recorded for the discarded diagnostics.
    /// Used by a speculative probe (e.g. resolving a generic's arguments to form a
    /// key) that discards its diagnostics: without releasing the once-guard, an
    /// authoritative re-resolution of the same type would be suppressed as a
    /// duplicate. Scoped narrowly so general truncation keeps its behavior.
    pub(crate) fn truncate_diagnostics_releasing_utility_keys(&mut self, len: usize) {
        if len < self.diagnostics.len() {
            for diagnostic in &self.diagnostics[len..] {
                let key = UtilityDiagnosticKey {
                    code: diagnostic.code.to_string(),
                    file_name: diagnostic.file_name.clone(),
                    span: diagnostic.span.map(|span| (span.start, span.end)),
                    message: diagnostic.message.clone(),
                };
                self.utility_diagnostic_keys.remove(&key);
            }
        }
        self.diagnostics.truncate(len);
    }

    pub(crate) fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub(crate) fn finish_with_stats(self) -> (Vec<Diagnostic>, CompatibilityStats) {
        (self.diagnostics, self.stats)
    }

    fn should_suppress(&self, diagnostic: &Diagnostic) -> bool {
        if self.options.diagnostic_profile == DiagnosticProfile::Native {
            return false;
        }

        if self.current_file_kind == FileKind::GeneratedDeclaration {
            return diagnostic.code.to_string() != "surge::parser-error";
        }

        // Physical default-lib files are trusted upstream declarations: never
        // surface diagnostics that originate inside them, so unsupported lib
        // syntax cannot flood normal user diagnostics.
        if self.current_file_kind == FileKind::PhysicalDefaultLib {
            return true;
        }

        let code = diagnostic.code.to_string();
        if code.starts_with("surge::") {
            return true;
        }

        if self.options.skip_lib_check && self.current_file_kind.is_declaration() {
            return true;
        }

        false
    }

    fn record_suppressed(&mut self, diagnostic: &Diagnostic) {
        self.stats.suppressed_diagnostics_total += 1;

        if self.current_file_kind.is_declaration() {
            self.stats.suppressed_declaration_diagnostics_total += 1;
        }

        if is_rust_only_compat_diagnostic(&diagnostic.code.to_string()) {
            self.stats.suppressed_rust_only_diagnostics_total += 1;
        }
    }
}

pub(crate) fn convert_span(span: SyntaxTextSpan) -> DiagnosticTextSpan {
    DiagnosticTextSpan {
        start: span.start,
        end: span.end,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UtilityDiagnosticKey {
    code: String,
    file_name: String,
    span: Option<(usize, usize)>,
    message: String,
}

fn is_rust_only_compat_diagnostic(code: &str) -> bool {
    code.starts_with("surge::")
}
