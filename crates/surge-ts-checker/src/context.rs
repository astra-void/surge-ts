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
    pub(crate) ambient_modules: Arc<std::collections::HashMap<String, ModuleExportTable>>,
    /// Module augmentations (`declare module "x"` in a file that is itself a
    /// module). Unlike ambient module declarations, these only merge into an
    /// already-resolved target; they do not make `"x"` resolvable on their own.
    pub(crate) module_augmentations: Arc<std::collections::HashMap<String, ModuleExportTable>>,
    pub(crate) ambient_global_symbols: SymbolTable,
    pub(crate) ambient_global_type_declarations: Arc<TypeDeclarationTable>,
    pub(crate) module_file_index_by_identity: Arc<HashMap<Arc<str>, usize>>,
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
    /// Lowest `resolving`-stack index that any cycle truncation has re-entered
    /// since this field was last reset. A resolution that pushed its declaration
    /// at stack depth `floor` is independent of the enclosing `resolving` context
    /// — and therefore safe to memoize — only if every cycle it triggered
    /// re-entered a frame at `floor` or deeper (an *internal* self/mutual cycle).
    /// A cycle reaching below `floor` means the result depends on an outer frame.
    /// See the generic instantiation cache in `resolve_named_type`.
    pub(crate) lowest_cycle_target_index: usize,
    file_kinds: Arc<HashMap<String, FileKind>>,
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
            ambient_modules: Arc::new(std::collections::HashMap::new()),
            module_augmentations: Arc::new(std::collections::HashMap::new()),
            ambient_global_symbols: SymbolTable::new(),
            ambient_global_type_declarations: Arc::new(TypeDeclarationTable::new()),
            module_file_index_by_identity: Arc::new(HashMap::new()),
            type_parameter_scopes: Vec::new(),
            type_parameter_constraint_scopes: Vec::new(),
            timings: None,
            namespace_member_resolution_depth: 0,
            lowest_cycle_target_index: usize::MAX,
            file_kinds: Arc::new(file_kinds),
        }
    }

    pub(crate) fn note_resolution_cycle(&mut self, target_index: usize) {
        self.lowest_cycle_target_index = self.lowest_cycle_target_index.min(target_index);
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

    /// Like [`lookup_type_declaration`](Self::lookup_type_declaration) but returns
    /// a [`TypeDeclarationHandle`] whose borrow is decoupled from `self`, so
    /// resolution can read the declaration while `self` is borrowed mutably
    /// without deep-cloning the payload.
    pub(crate) fn lookup_type_declaration_handle(
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
