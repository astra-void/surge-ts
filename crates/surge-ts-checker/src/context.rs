use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use surge_ts_diagnostics::{Diagnostic, TextSpan as DiagnosticTextSpan};
use surge_ts_syntax::{ParsedType, ParsedTypeParameter, TextSpan as SyntaxTextSpan};
use surge_ts_types::fx::FxHashMap;
use surge_ts_types::{FunctionType, ProgramTypeStore, Type, current_program_type_store};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

static NEXT_DECLARATION_ENVIRONMENT_OWNER: AtomicU32 = AtomicU32::new(1);
static NEXT_SUBSTITUTION_STORE_OWNER: AtomicU32 = AtomicU32::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DeclarationEnvironmentId(u64);

impl DeclarationEnvironmentId {
    fn new(owner: u32, index: u32) -> Self {
        Self((u64::from(owner) << 32) | u64::from(index))
    }

    fn owner(self) -> u32 {
        (self.0 >> 32) as u32
    }

    fn index(self) -> usize {
        (self.0 as u32).saturating_sub(1) as usize
    }
}

/// Content-stable identity of one `resolved_named_types` memo map instance.
/// Pointer identity is regime-dependent (a parallel worker's fresh context
/// creates different map instances than the serial rolling context), so map
/// instances are identified by *where the program created them*: the file
/// whose body window created the map, the deterministic resolution-stage
/// counter, the within-body ordinal (body start / mid-body / shadow), and the
/// speculative-attempt tag (0 = first attempt, 1 = STC recheck, which must
/// not collide with the discarded speculative attempt's environments).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EnvironmentMapIdentity {
    creator: Arc<str>,
    stage: u64,
    ordinal: u32,
    attempt: u64,
}

impl EnvironmentMapIdentity {
    fn initial() -> Self {
        Self {
            creator: Arc::from(""),
            stage: 0,
            ordinal: 0,
            attempt: 0,
        }
    }
}

/// Dedup key for interned declaration environments. Every component is
/// content-derived (never a bare allocation address), so two contexts in the
/// same semantic state — a parallel worker's fresh clone and the serial
/// rolling context at the same module — intern to the same environment and,
/// critically, produce the same canonicalization discriminator: lazy
/// `Type::Reference`s compare equal across regimes exactly when serial
/// semantics say they should (see `program_canonicalization_discriminator`
/// consumers in the canonical type store).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeclarationEnvironmentKey {
    file_name: String,
    file_kind: FileKind,
    /// `(instance_id, version)` per scope layer, in layer order; `None` scope
    /// is distinguished from an empty layer list.
    has_scope: bool,
    scope_layers: Vec<(u64, u64)>,
    resolved_named_types_identity: EnvironmentMapIdentity,
    /// The per-file module maps are stage-installed shared `Arc`s (regime
    /// stable); an empty map is identified as 0 regardless of which default
    /// `Arc` instance holds it (`mem::take` windows create fresh empties).
    module_scope_identity: usize,
    module_values_identity: usize,
    /// `(instance_id, version)` of the context's live declaration table. The
    /// pointer scheme guaranteed key-equal interns shared one table (same
    /// memo-map burst implied same table); the content key must carry it
    /// explicitly or environments from different table contexts merge and
    /// first-wins data capture materializes the wrong table.
    type_declarations_identity: (u64, u64),
    /// Deterministic stage counter at intern time: distinguishes re-visits of
    /// a file across stage boundaries that share a carried memo map.
    stage_at_intern: u64,
    /// File-switch ordinal within the current anchor window (window = last
    /// memo-map replacement or stage boundary): the regime-stable
    /// reconstruction of the old rolling generation, which separated fixpoint
    /// re-entries of the same file — observable through which re-entry's
    /// references canonicalize together.
    visit: u64,
}

fn environment_content_discriminator(key: &DeclarationEnvironmentKey) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = surge_ts_types::fx::FxHasher::default();
    key.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone)]
struct DeclarationEnvironmentData {
    file_name: String,
    current_file_kind: FileKind,
    options: Arc<CheckerOptions>,
    symbols: SymbolTable,
    /// Immutable snapshot of the capturing context's `type_declarations`.
    /// Shared across every environment interned while the live table is
    /// unmutated (see [`DeclarationEnvironmentStore::snapshot_type_declarations`]);
    /// tens of thousands of environments would otherwise each own a full index
    /// copy of their module's declaration table.
    type_declarations: Arc<TypeDeclarationTable>,
    type_declaration_scope: Option<Arc<TypeDeclarationScope>>,
    program_type_store: Arc<ProgramTypeStore>,
    substitution_store: Arc<SubstitutionStore>,
    resolved_named_types:
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, DeclarationResolutionState>>>,
    program_resolved_generic_types:
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, Vec<GenericInstantiationCacheEntry>>>>,
    program_instantiations:
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, Vec<InstantiationCacheEntry>>>>,
    physical_interface_instantiations: Arc<Mutex<FxHashMap<InterfaceInstantiationKey, Arc<Type>>>>,
    physical_interface_declaration_templates:
        Arc<Mutex<FxHashMap<StableInterfaceDeclarationId, Arc<InterfaceDeclarationTemplate>>>>,
    physical_interface_method_instantiations:
        Arc<Mutex<FxHashMap<InterfaceMemberInstantiationKey, FunctionType>>>,
    physical_interface_overload_instantiations:
        Arc<Mutex<FxHashMap<InterfaceOverloadInstantiationKey, FunctionType>>>,
    ambient_modules: Arc<FxHashMap<String, ModuleExportTable>>,
    module_augmentations: Arc<FxHashMap<String, ModuleExportTable>>,
    ambient_global_symbols: SymbolTable,
    ambient_global_type_declarations: Arc<TypeDeclarationTable>,
    module_file_index_by_identity: Arc<FxHashMap<Arc<str>, usize>>,
    module_scope_by_file: Arc<FxHashMap<Arc<str>, Arc<TypeDeclarationScope>>>,
    module_local_values_by_file: Arc<FxHashMap<Arc<str>, Arc<SymbolTable>>>,
    jsx_intrinsic_elements_declarer: Option<(Arc<TypeDeclarationTable>, String)>,
    type_parameter_scopes: Vec<HashMap<String, Type>>,
    type_parameter_constraint_scopes: Vec<HashMap<String, ParsedType>>,
    timings: Option<Arc<Mutex<ProgramTimings>>>,
    file_kinds: Arc<FxHashMap<String, FileKind>>,
    module_value_fallback: Option<Arc<SymbolTable>>,
    resolved_named_types_identity: EnvironmentMapIdentity,
    resolution_stage_counter: u64,
    environment_attempt: u64,
    environment_visit_counter: u64,
}

#[derive(Debug)]
pub(crate) struct DeclarationEnvironmentStore {
    owner: u32,
    next_index: AtomicU32,
    requests: AtomicU64,
    hits: AtomicU64,
    entries: Mutex<DeclarationEnvironmentEntries>,
    /// `(instance_id, version)` → snapshot memo for the most recent
    /// `type_declarations` capture. Environments are interned in bursts between
    /// table mutations, so one snapshot serves the whole burst.
    type_declarations_snapshot: Mutex<Option<((u64, u64), Arc<TypeDeclarationTable>)>>,
}

#[derive(Debug, Default)]
struct DeclarationEnvironmentEntries {
    by_key: HashMap<DeclarationEnvironmentKey, (DeclarationEnvironmentId, u64)>,
    by_id: Vec<Arc<DeclarationEnvironmentData>>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclarationEnvironmentHandle {
    id: DeclarationEnvironmentId,
    /// Content hash of the environment's dedup key: equal for semantically
    /// identical environments regardless of context instance or intern order.
    discriminator: u64,
    store: Weak<DeclarationEnvironmentStore>,
}

impl DeclarationEnvironmentStore {
    fn new() -> Arc<Self> {
        let owner = NEXT_DECLARATION_ENVIRONMENT_OWNER.fetch_add(1, Ordering::Relaxed);
        assert_ne!(owner, 0, "declaration-environment owner space exhausted");
        Arc::new(Self {
            owner,
            next_index: AtomicU32::new(1),
            requests: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            entries: Mutex::new(DeclarationEnvironmentEntries::default()),
            type_declarations_snapshot: Mutex::new(None),
        })
    }

    /// Returns a shared immutable snapshot of `ctx.type_declarations`, reusing
    /// the previous snapshot while the exact same table instance is unmutated.
    fn snapshot_type_declarations(&self, ctx: &CheckerContext) -> Arc<TypeDeclarationTable> {
        let identity = ctx.type_declarations.snapshot_identity();
        let mut memo = self
            .type_declarations_snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((cached_identity, snapshot)) = memo.as_ref()
            && *cached_identity == identity
        {
            return snapshot.clone();
        }
        let snapshot = Arc::new(ctx.type_declarations.clone());
        *memo = Some((identity, snapshot.clone()));
        snapshot
    }

    fn intern(self: &Arc<Self>, ctx: &CheckerContext) -> DeclarationEnvironmentHandle {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let key = DeclarationEnvironmentKey {
            file_name: ctx.file_name.clone(),
            file_kind: ctx.current_file_kind,
            has_scope: ctx.type_declaration_scope.is_some(),
            scope_layers: ctx
                .type_declaration_scope
                .as_ref()
                .map_or_else(Vec::new, |scope| {
                    scope
                        .layers()
                        .iter()
                        .map(|layer| layer.snapshot_identity())
                        .collect()
                }),
            resolved_named_types_identity: ctx.resolved_named_types_identity.clone(),
            module_scope_identity: if ctx.module_scope_by_file.is_empty() {
                0
            } else {
                Arc::as_ptr(&ctx.module_scope_by_file) as usize
            },
            module_values_identity: if ctx.module_local_values_by_file.is_empty() {
                0
            } else {
                Arc::as_ptr(&ctx.module_local_values_by_file) as usize
            },
            type_declarations_identity: ctx.type_declarations.snapshot_identity(),
            stage_at_intern: ctx.resolution_stage_counter,
            visit: ctx.environment_visit_counter,
        };
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((id, discriminator)) = entries.by_key.get(&key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return DeclarationEnvironmentHandle {
                id: *id,
                discriminator: *discriminator,
                store: Arc::downgrade(self),
            };
        }
        let id = DeclarationEnvironmentId::new(
            self.owner,
            self.next_index.fetch_add(1, Ordering::Relaxed),
        );
        let discriminator = environment_content_discriminator(&key);
        let data = Arc::new(DeclarationEnvironmentData::capture(
            ctx,
            self.snapshot_type_declarations(ctx),
        ));
        debug_assert_eq!(id.index(), entries.by_id.len());
        entries.by_key.insert(key, (id, discriminator));
        entries.by_id.push(data);
        DeclarationEnvironmentHandle {
            id,
            discriminator,
            store: Arc::downgrade(self),
        }
    }

    pub(crate) fn stats(&self) -> (u64, u64, u64) {
        (
            self.requests.load(Ordering::Relaxed),
            self.hits.load(Ordering::Relaxed),
            self.entries
                .lock()
                .map(|entries| entries.by_id.len() as u64)
                .unwrap_or_default(),
        )
    }

    /// Census-only iteration over the interned environments, exposing the
    /// owned captures a retained-memory walk needs to attribute.
    pub(crate) fn census_environments(
        &self,
        f: &mut dyn FnMut(
            &str,
            &SymbolTable,
            &TypeDeclarationTable,
            Option<&Arc<TypeDeclarationScope>>,
            usize,
        ),
    ) {
        let Ok(entries) = self.entries.lock() else {
            return;
        };
        for data in &entries.by_id {
            let type_parameter_scope_entries = data
                .type_parameter_scopes
                .iter()
                .map(HashMap::len)
                .sum::<usize>()
                + data
                    .type_parameter_constraint_scopes
                    .iter()
                    .map(HashMap::len)
                    .sum::<usize>();
            f(
                &data.file_name,
                &data.symbols,
                &data.type_declarations,
                data.type_declaration_scope.as_ref(),
                type_parameter_scope_entries,
            );
        }
    }
}

impl DeclarationEnvironmentHandle {
    pub(crate) fn checker_context(&self) -> Option<CheckerContext> {
        let store = self.store.upgrade()?;
        if self.id.owner() != store.owner {
            return None;
        }
        let data = store
            .entries
            .lock()
            .ok()?
            .by_id
            .get(self.id.index())
            .cloned()?;
        Some(CheckerContext::from_declaration_environment(&data, store))
    }

    pub(crate) fn canonicalization_discriminator(&self) -> u64 {
        self.discriminator
    }
}

impl DeclarationEnvironmentData {
    fn capture(ctx: &CheckerContext, type_declarations: Arc<TypeDeclarationTable>) -> Self {
        Self {
            file_name: ctx.file_name.clone(),
            current_file_kind: ctx.current_file_kind,
            options: ctx.options.clone(),
            // EXPERIMENT(env-symbols): drop the working value-table capture;
            // typeof falls back to ambient globals / module_value_fallback /
            // module_local_values_by_file.
            symbols: SymbolTable::new(),
            type_declarations,
            type_declaration_scope: ctx.type_declaration_scope.clone(),
            program_type_store: ctx.program_type_store.clone(),
            substitution_store: ctx.substitution_store.clone(),
            resolved_named_types: ctx.resolved_named_types.clone(),
            program_resolved_generic_types: ctx.program_resolved_generic_types.clone(),
            program_instantiations: ctx.program_instantiations.clone(),
            physical_interface_instantiations: ctx.physical_interface_instantiations.clone(),
            physical_interface_declaration_templates: ctx
                .physical_interface_declaration_templates
                .clone(),
            physical_interface_method_instantiations: ctx
                .physical_interface_method_instantiations
                .clone(),
            physical_interface_overload_instantiations: ctx
                .physical_interface_overload_instantiations
                .clone(),
            ambient_modules: ctx.ambient_modules.clone(),
            module_augmentations: ctx.module_augmentations.clone(),
            ambient_global_symbols: ctx.ambient_global_symbols.clone_for_environment_capture(),
            ambient_global_type_declarations: ctx.ambient_global_type_declarations.clone(),
            module_file_index_by_identity: ctx.module_file_index_by_identity.clone(),
            module_scope_by_file: ctx.module_scope_by_file.clone(),
            module_local_values_by_file: ctx.module_local_values_by_file.clone(),
            jsx_intrinsic_elements_declarer: ctx.jsx_intrinsic_elements_declarer.clone(),
            type_parameter_scopes: ctx.type_parameter_scopes.clone(),
            type_parameter_constraint_scopes: ctx.type_parameter_constraint_scopes.clone(),
            timings: ctx.timings.clone(),
            file_kinds: ctx.file_kinds.clone(),
            resolved_named_types_identity: ctx.resolved_named_types_identity.clone(),
            resolution_stage_counter: ctx.resolution_stage_counter,
            environment_attempt: ctx.environment_attempt,
            environment_visit_counter: ctx.environment_visit_counter,
            module_value_fallback: ctx.module_value_fallback.clone(),
        }
    }
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
    /// Count of non-relative (package) import/export specifiers that failed every
    /// resolution path — the subset of `externalModuleStubs` references that were
    /// actually unresolved (and either stubbed or reported TS2307), rather than
    /// resolved via a dependency declaration. This is the parity-relevant figure:
    /// a resolved external reference is benign, an unresolved one is the risk.
    pub external_modules_unresolved_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
    pub no_implicit_returns: bool,
    pub no_fallthrough_cases_in_switch: bool,
    pub no_implicit_override: bool,
    pub no_property_access_from_index_signature: bool,
    pub no_unused_locals: bool,
    pub no_unused_parameters: bool,
    pub stub_external_modules: bool,
    pub resolved_modules: FxHashMap<String, String>,
    /// Importer-scoped module resolutions: canonical importer file name →
    /// specifier → resolved file. Consulted before [`Self::resolved_modules`]
    /// so two importers can resolve the same bare specifier to different files
    /// (nested `node_modules`, package `#imports` scopes, self-name imports).
    /// `resolved_modules` stays as the project-wide fallback: `paths`/`baseUrl`
    /// mappings are importer-independent, and older callers only populate the
    /// flat map.
    pub resolved_modules_by_importer: FxHashMap<String, FxHashMap<String, String>>,
    /// Effective type-package names included in the program. When the project's
    /// `compilerOptions.types` used the `"*"` wildcard, the literal `"*"` is kept
    /// in this list as a sentinel (see [`Self::types_uses_wildcard`]); it never
    /// matches a real `@types` package path, so the other consumers ignore it.
    pub types: Vec<String>,
    pub no_lib: bool,
    pub skip_lib_check: bool,
    /// `jsx: react-jsx`/`react-jsxdev`: the JSX namespace resolves through the
    /// automatic runtime module, so intrinsic elements type-check without a
    /// `React` binding in scope. Under `preserve`/classic modes tsc requires the
    /// factory namespace to be visible, so the runtime fallback must not fire.
    pub jsx_automatic_runtime: bool,
    pub diagnostic_profile: DiagnosticProfile,
}

impl CheckerOptions {
    pub const ALLOW_SYNTHETIC_DEFAULT_IMPORTS_SENTINEL: &'static str =
        "\0allowSyntheticDefaultImports";

    /// Whether `compilerOptions.types` contained the `"*"` wildcard. Selects the
    /// node install-hint variant (TS2580 with a wildcard, TS2591 without),
    /// matching TypeScript's `usesWildcardTypes` branch.
    pub(crate) fn types_uses_wildcard(&self) -> bool {
        self.types.iter().any(|name| name == "*")
    }

    pub(crate) fn allow_synthetic_default_imports(&self) -> bool {
        self.resolved_modules
            .contains_key(Self::ALLOW_SYNTHETIC_DEFAULT_IMPORTS_SENTINEL)
    }

    /// Resolve `specifier` from `importer_file`, preferring the importer-scoped
    /// map. Falls back to the project-wide map so paths/baseUrl mappings and
    /// flat-map callers keep working.
    pub(crate) fn resolved_module_for(
        &self,
        importer_file: &str,
        specifier: &str,
    ) -> Option<&String> {
        if let Some(per_importer) = self.resolved_modules_by_importer.get(importer_file) {
            if let Some(resolved) = per_importer.get(specifier) {
                return Some(resolved);
            }
        }
        self.resolved_modules.get(specifier)
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
            no_unused_locals: false,
            no_unused_parameters: false,
            stub_external_modules: false,
            resolved_modules: FxHashMap::default(),
            resolved_modules_by_importer: FxHashMap::default(),
            types: Vec::new(),
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
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
    pub(crate) file_name: Arc<str>,
    pub(crate) name: Arc<str>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StableInterfaceDeclarationFragmentId {
    pub(crate) canonical_file: Arc<str>,
    pub(crate) declaration_start: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StableInterfaceDeclarationId {
    pub(crate) canonical_file: Arc<str>,
    pub(crate) declaration_start: u32,
    pub(crate) declaration_name: Arc<str>,
    pub(crate) merged_fragments: Arc<[StableInterfaceDeclarationFragmentId]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CanonicalTypeIdentity {
    String,
    Number,
    Boolean,
    BigInt,
    Symbol,
    Undefined,
    Void,
    Any,
    Never,
    StringLiteral(Arc<str>),
    NumberLiteral(Arc<str>),
    BooleanLiteral(bool),
    Array(Box<Self>),
    Tuple(Arc<[Self]>),
    Reference {
        declaration: Arc<str>,
        arguments: Arc<[Self]>,
    },
    NamedObject(Arc<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SubstitutionId(u64);

impl SubstitutionId {
    fn new(owner: u32, index: u32) -> Self {
        Self((u64::from(owner) << 32) | u64::from(index))
    }
}

#[derive(Debug)]
struct SubstitutionEntry {
    declaration: StableInterfaceDeclarationId,
    arguments: Arc<[CanonicalTypeIdentity]>,
    id: SubstitutionId,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SubstitutionStoreStats {
    pub(crate) requests: u64,
    pub(crate) hits: u64,
    pub(crate) unique: u64,
    pub(crate) input_arguments: u64,
    pub(crate) stored_arguments: u64,
    pub(crate) argument_storage_avoided: u64,
}

#[derive(Debug)]
pub(crate) struct SubstitutionStore {
    owner: u32,
    next_index: AtomicU32,
    requests: AtomicU64,
    hits: AtomicU64,
    input_arguments: AtomicU64,
    stored_arguments: AtomicU64,
    argument_storage_avoided: AtomicU64,
    entries: Mutex<HashMap<u64, Vec<SubstitutionEntry>>>,
}

impl SubstitutionStore {
    fn new() -> Arc<Self> {
        let owner = NEXT_SUBSTITUTION_STORE_OWNER.fetch_add(1, Ordering::Relaxed);
        assert_ne!(owner, 0, "substitution-store owner space exhausted");
        Arc::new(Self {
            owner,
            next_index: AtomicU32::new(1),
            requests: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            input_arguments: AtomicU64::new(0),
            stored_arguments: AtomicU64::new(0),
            argument_storage_avoided: AtomicU64::new(0),
            entries: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn intern(
        &self,
        declaration: StableInterfaceDeclarationId,
        arguments: Vec<CanonicalTypeIdentity>,
    ) -> SubstitutionId {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.input_arguments
            .fetch_add(arguments.len() as u64, Ordering::Relaxed);
        let argument_count = arguments.len() as u64;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        declaration.hash(&mut hasher);
        arguments.hash(&mut hasher);
        let key = hasher.finish();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let bucket = entries.entry(key).or_default();
        for entry in bucket.iter() {
            if entry.declaration == declaration && entry.arguments.as_ref() == arguments.as_slice()
            {
                self.hits.fetch_add(1, Ordering::Relaxed);
                self.argument_storage_avoided
                    .fetch_add(argument_count, Ordering::Relaxed);
                return entry.id;
            }
        }
        let id = SubstitutionId::new(self.owner, self.next_index.fetch_add(1, Ordering::Relaxed));
        self.stored_arguments
            .fetch_add(argument_count, Ordering::Relaxed);
        bucket.push(SubstitutionEntry {
            declaration,
            arguments: Arc::from(arguments),
            id,
        });
        id
    }

    pub(crate) fn stats(&self) -> SubstitutionStoreStats {
        SubstitutionStoreStats {
            requests: self.requests.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            unique: self
                .entries
                .lock()
                .map(|entries| entries.values().map(Vec::len).sum::<usize>() as u64)
                .unwrap_or_default(),
            input_arguments: self.input_arguments.load(Ordering::Relaxed),
            stored_arguments: self.stored_arguments.load(Ordering::Relaxed),
            argument_storage_avoided: self.argument_storage_avoided.load(Ordering::Relaxed),
        }
    }

    fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceEnvironmentIdentity {
    pub(crate) no_lib: bool,
    pub(crate) skip_lib_check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceInstantiationKey {
    pub(crate) declaration: StableInterfaceDeclarationId,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InterfaceMemberDeclarationKind {
    Property,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StableInterfaceMemberDeclarationId {
    pub(crate) containing_interface: StableInterfaceDeclarationId,
    pub(crate) canonical_file: Arc<str>,
    pub(crate) declaration_start: u32,
    pub(crate) declaration_kind: InterfaceMemberDeclarationKind,
    pub(crate) declared_name: Arc<str>,
    pub(crate) overload_index: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceMemberDeclarationTemplate {
    pub(crate) declaration: StableInterfaceMemberDeclarationId,
    pub(crate) overload_group: Option<u32>,
    pub(crate) overload_position: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceMethodOverloadGroupTemplate {
    pub(crate) ordered_members: Arc<[StableInterfaceMemberDeclarationId]>,
}

#[derive(Debug, Clone)]
pub(crate) struct InterfaceDeclarationTemplate {
    pub(crate) members: Arc<[InterfaceMemberDeclarationTemplate]>,
    pub(crate) method_groups: Arc<[InterfaceMethodOverloadGroupTemplate]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceMemberInstantiationKey {
    pub(crate) member: StableInterfaceMemberDeclarationId,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceOverloadInstantiationKey {
    pub(crate) containing_interface: StableInterfaceDeclarationId,
    pub(crate) ordered_members: Arc<[StableInterfaceMemberDeclarationId]>,
    pub(crate) prefix_len: u32,
    pub(crate) substitution: SubstitutionId,
    pub(crate) environment: InterfaceEnvironmentIdentity,
}

#[derive(Debug, Clone)]
pub(crate) struct CheckerContext {
    pub(crate) file_name: String,
    pub(crate) current_file_kind: FileKind,
    pub(crate) options: Arc<CheckerOptions>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    // Dedup index for `push`, mirroring the keys of `diagnostics`. `push` rejected
    // duplicates by scanning the whole `diagnostics` vec (re-rendering every code
    // to a `String` per comparison), so a context that emits D diagnostics was
    // O(D^2) — e.g. a single file with thousands of unresolved-name reports. The
    // set makes the check O(1); `diagnostic_keys_len` lets `push` detect when
    // `diagnostics` was mutated directly (clear/take/truncate) and rebuild lazily.
    diagnostic_keys: HashSet<
        (
            String,
            String,
            String,
            Option<surge_ts_diagnostics::TextSpan>,
        ),
        surge_ts_types::fx::FxBuildHasher,
    >,
    diagnostic_keys_len: usize,
    pub(crate) stats: CompatibilityStats,
    /// File-region overlay of `push_utility_diagnostic_once` keys: entries
    /// recorded since the last `begin_file_check`. Keys inherited from before
    /// the check phase live in the shared baseline below, so the per-file
    /// reset is an O(overlay) clear instead of a full-set clone.
    pub(crate) utility_diagnostic_keys:
        HashSet<UtilityDiagnosticKey, surge_ts_types::fx::FxBuildHasher>,
    /// The utility-key set this worker context entered the check phase with,
    /// captured (not cloned) on the first `begin_file_check`. Suppression
    /// consults baseline then overlay, which is exactly the pre-region
    /// semantics where both lived in one set. See [`Self::begin_file_check`].
    utility_diagnostic_keys_baseline:
        Option<Arc<HashSet<UtilityDiagnosticKey, surge_ts_types::fx::FxBuildHasher>>>,
    pub(crate) symbols: SymbolTable,
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) type_declaration_scope: Option<Arc<TypeDeclarationScope>>,
    pub(crate) program_type_store: Arc<ProgramTypeStore>,
    pub(crate) substitution_store: Arc<SubstitutionStore>,
    pub(crate) declaration_environment_store: Arc<DeclarationEnvironmentStore>,
    declaration_environment_generation: u64,
    /// See [`EnvironmentMapIdentity`]: content-stable identity of the current
    /// `resolved_named_types` instance, updated at every replacement site.
    pub(crate) resolved_named_types_identity: EnvironmentMapIdentity,
    /// Deterministic count of main-thread resolution stages entered
    /// ([`Self::begin_resolution_stage`]); identical in serial and parallel
    /// modes because stage transitions happen on the linear driver path.
    pub(crate) resolution_stage_counter: u64,
    /// 0 for first-attempt work, 1 while re-running a file/module in the STC
    /// commit walk, so recheck environments never collide with the discarded
    /// speculative attempt's.
    pub(crate) environment_attempt: u64,
    /// See `DeclarationEnvironmentKey::visit`: bumped on every file switch,
    /// reset at every memo-map replacement and stage boundary.
    environment_visit_counter: u64,
    pub(crate) resolved_named_types:
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, DeclarationResolutionState>>>,
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
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, Vec<GenericInstantiationCacheEntry>>>>,
    /// Program-wide instantiation interner backing lazy/nominal `Type::Reference`
    /// resolution. Maps a declaration + resolved type arguments to the shared
    /// structural expansion, so a reference resolves (and the body expands) at
    /// most once per unique instantiation rather than at every use site. Shared
    /// via `Arc` across all `CheckerContext` clones and jobs.
    pub(crate) program_instantiations:
        Arc<Mutex<FxHashMap<DeclarationResolutionKey, Vec<InstantiationCacheEntry>>>>,
    /// Completed, clean physical-default-lib interface expansions keyed without
    /// structural `Type` equality. Unlike `program_instantiations`, this index is
    /// eligible inside an enclosing generic scope when every actual argument has
    /// a stable nominal/literal identity.
    pub(crate) physical_interface_instantiations:
        Arc<Mutex<FxHashMap<InterfaceInstantiationKey, Arc<Type>>>>,
    pub(crate) physical_interface_declaration_templates:
        Arc<Mutex<FxHashMap<StableInterfaceDeclarationId, Arc<InterfaceDeclarationTemplate>>>>,
    pub(crate) physical_interface_method_instantiations:
        Arc<Mutex<FxHashMap<InterfaceMemberInstantiationKey, FunctionType>>>,
    pub(crate) physical_interface_overload_instantiations:
        Arc<Mutex<FxHashMap<InterfaceOverloadInstantiationKey, FunctionType>>>,
    pub(crate) ambient_modules: Arc<FxHashMap<String, ModuleExportTable>>,
    /// Module augmentations (`declare module "x"` in a file that is itself a
    /// module). Unlike ambient module declarations, these only merge into an
    /// already-resolved target; they do not make `"x"` resolvable on their own.
    pub(crate) module_augmentations: Arc<FxHashMap<String, ModuleExportTable>>,
    pub(crate) ambient_global_symbols: SymbolTable,
    pub(crate) ambient_global_type_declarations: Arc<TypeDeclarationTable>,
    pub(crate) module_file_index_by_identity: Arc<FxHashMap<Arc<str>, usize>>,
    /// Each module's resolution scope keyed by its source `file_name`, mirroring
    /// `shared_state.module_resolution_scopes` but addressable by name. A type
    /// alias/interface imported across a module *import cycle* can lose its
    /// pre-attached `resolution_scope` when the multi-pass binding fixpoint rebinds
    /// it (the source module's scope is not yet available in that pass), leaving it
    /// `None`. Resolving such a declaration's body must still happen in its
    /// declaring module's scope, so resolution falls back to this map keyed by the
    /// declaration's `file_name`. Refreshed before each module-analysis round with
    /// the freshest resolution scopes (signature collection resolves parameter
    /// types through local aliases whose attached scope lacks import layers), then
    /// set a final time before the check phase.
    pub(crate) module_scope_by_file: Arc<FxHashMap<Arc<str>, Arc<TypeDeclarationScope>>>,
    /// Each module's local value symbols, keyed by `file_name`. The value analogue
    /// of [`Self::module_scope_by_file`]: when an imported type alias's body is
    /// resolved while checking a *consumer* file, a `typeof <localValue>` in that
    /// body must resolve against the *declaring* module's values, not the
    /// consumer's `symbols`. Populated once (read-only) before the check phase and
    /// shared across jobs. Consulted via `get` only.
    pub(crate) module_local_values_by_file: Arc<FxHashMap<Arc<str>, Arc<SymbolTable>>>,
    /// The export-table type declarations of the module that exports the
    /// program's JSX intrinsic-elements interface, plus its key in that table
    /// (`JSX.IntrinsicElements`), located once after module binding. Under the
    /// automatic runtime (`jsx: react-jsx`) the JSX checker resolves intrinsic
    /// tags through this table when no `JSX`/`React.JSX` binding is visible from
    /// the consuming file — tsc reaches the namespace through the runtime module
    /// import it synthesizes.
    pub(crate) jsx_intrinsic_elements_declarer: Option<(Arc<TypeDeclarationTable>, String)>,
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
    /// Depth of `with_file_name` frames whose file differs from the enclosing
    /// one — nonzero exactly while a declaration from another file is being
    /// resolved. See [`Self::lookup_ignores_local_table`].
    pub(crate) cross_file_resolution_depth: u32,
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
    /// `resolving`-stack indices of the frames that cross a structural type —
    /// an interface body, or a type alias whose body is itself structural
    /// (object/array/function/…). A type-alias cycle whose re-entry path passes
    /// through one of these frames is tsc-legal recursion (`type Issue = A |
    /// InvalidUnion` where `InvalidUnion.errors: Issue[][]`), not a
    /// structureless `type A = B; type B = A` cycle. Frames are recorded by
    /// `resolve_interface`/`resolve_type_alias` for the duration of their body
    /// resolution.
    pub(crate) structural_resolution_frames: Vec<usize>,
    file_kinds: Arc<FxHashMap<String, FileKind>>,
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
        file_kinds: FxHashMap<String, FileKind>,
    ) -> Self {
        Self::new_with_shared_options(file_name, Arc::new(options), file_kinds)
    }

    /// Like [`Self::new`], but shares an existing options handle. The options
    /// carry the project-wide module-resolution tables
    /// (`resolved_modules_by_importer`), so per-module shadow contexts must not
    /// deep-clone them.
    pub(crate) fn new_with_shared_options(
        file_name: String,
        options: Arc<CheckerOptions>,
        file_kinds: FxHashMap<String, FileKind>,
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
            diagnostic_keys: HashSet::default(),
            diagnostic_keys_len: 0,
            stats: CompatibilityStats::default(),
            utility_diagnostic_keys: HashSet::default(),
            utility_diagnostic_keys_baseline: None,
            symbols: SymbolTable::new(),
            type_declarations: TypeDeclarationTable::new(),
            type_declaration_scope: None,
            program_type_store: current_program_type_store().unwrap_or_else(ProgramTypeStore::new),
            substitution_store: SubstitutionStore::new(),
            declaration_environment_store: DeclarationEnvironmentStore::new(),
            declaration_environment_generation: 0,
            resolved_named_types_identity: EnvironmentMapIdentity::initial(),
            resolution_stage_counter: 0,
            environment_attempt: 0,
            environment_visit_counter: 0,
            resolved_named_types: Arc::new(Mutex::new(FxHashMap::default())),
            program_resolved_generic_types: Arc::new(Mutex::new(FxHashMap::default())),
            program_instantiations: Arc::new(Mutex::new(FxHashMap::default())),
            physical_interface_instantiations: Arc::new(Mutex::new(FxHashMap::default())),
            physical_interface_declaration_templates: Arc::new(Mutex::new(FxHashMap::default())),
            physical_interface_method_instantiations: Arc::new(Mutex::new(FxHashMap::default())),
            physical_interface_overload_instantiations: Arc::new(Mutex::new(FxHashMap::default())),
            ambient_modules: Arc::new(FxHashMap::default()),
            module_augmentations: Arc::new(FxHashMap::default()),
            ambient_global_symbols: SymbolTable::new(),
            ambient_global_type_declarations: Arc::new(TypeDeclarationTable::new()),
            module_file_index_by_identity: Arc::new(FxHashMap::default()),
            module_scope_by_file: Arc::new(FxHashMap::default()),
            module_local_values_by_file: Arc::new(FxHashMap::default()),
            jsx_intrinsic_elements_declarer: None,
            type_parameter_scopes: Vec::new(),
            type_parameter_constraint_scopes: Vec::new(),
            timings: None,
            namespace_member_resolution_depth: 0,
            cross_file_resolution_depth: 0,
            namespace_member_prefix_stack: Vec::new(),
            lowest_cycle_target_index: usize::MAX,
            structural_resolution_frames: Vec::new(),
            file_kinds: Arc::new(file_kinds),
            module_value_fallback: None,
        }
    }

    /// Replaces the named-resolution memo with a fresh map and records its
    /// content-stable identity: (current file, current resolution stage,
    /// `ordinal`, current attempt). Every site that swaps the map must go
    /// through here so environment identity stays pointer-free.
    pub(crate) fn replace_resolved_named_types(&mut self, ordinal: u32) {
        self.resolved_named_types = Arc::new(Mutex::new(FxHashMap::default()));
        self.resolved_named_types_identity = EnvironmentMapIdentity {
            creator: Arc::from(self.file_name.as_str()),
            stage: self.resolution_stage_counter,
            ordinal,
            attempt: self.environment_attempt,
        };
        self.environment_visit_counter = 0;
    }

    /// Marks a main-thread resolution stage boundary: bumps the deterministic
    /// stage counter so map identities created in different stages never
    /// collide. The carried memo map itself is deliberately left in place —
    /// stage-time resolution hitting the previous stage's memo is observable
    /// serial behavior (one tRPC display depends on it), so parallel drivers
    /// must instead reproduce the serial carry (see the last-committed-module
    /// hand-off in `collect_module_analyses_with_bindings_parallel`).
    pub(crate) fn begin_resolution_stage(&mut self) {
        self.resolution_stage_counter += 1;
        self.environment_visit_counter = 0;
    }

    pub(crate) fn note_resolution_cycle(&mut self, target_index: usize) {
        self.lowest_cycle_target_index = self.lowest_cycle_target_index.min(target_index);
    }

    pub(crate) fn declaration_environment(&self) -> DeclarationEnvironmentHandle {
        self.declaration_environment_store.intern(self)
    }

    fn from_declaration_environment(
        data: &DeclarationEnvironmentData,
        declaration_environment_store: Arc<DeclarationEnvironmentStore>,
    ) -> Self {
        Self {
            file_name: data.file_name.clone(),
            current_file_kind: data.current_file_kind,
            options: data.options.clone(),
            diagnostics: Vec::new(),
            diagnostic_keys: HashSet::default(),
            diagnostic_keys_len: 0,
            stats: CompatibilityStats::default(),
            utility_diagnostic_keys: HashSet::default(),
            utility_diagnostic_keys_baseline: None,
            symbols: data.symbols.clone(),
            type_declarations: data.type_declarations.as_ref().clone(),
            type_declaration_scope: data.type_declaration_scope.clone(),
            program_type_store: data.program_type_store.clone(),
            substitution_store: data.substitution_store.clone(),
            declaration_environment_store,
            declaration_environment_generation: 0,
            resolved_named_types_identity: data.resolved_named_types_identity.clone(),
            resolution_stage_counter: data.resolution_stage_counter,
            environment_attempt: data.environment_attempt,
            environment_visit_counter: data.environment_visit_counter,
            resolved_named_types: data.resolved_named_types.clone(),
            program_resolved_generic_types: data.program_resolved_generic_types.clone(),
            program_instantiations: data.program_instantiations.clone(),
            physical_interface_instantiations: data.physical_interface_instantiations.clone(),
            physical_interface_declaration_templates: data
                .physical_interface_declaration_templates
                .clone(),
            physical_interface_method_instantiations: data
                .physical_interface_method_instantiations
                .clone(),
            physical_interface_overload_instantiations: data
                .physical_interface_overload_instantiations
                .clone(),
            ambient_modules: data.ambient_modules.clone(),
            module_augmentations: data.module_augmentations.clone(),
            ambient_global_symbols: data.ambient_global_symbols.clone(),
            ambient_global_type_declarations: data.ambient_global_type_declarations.clone(),
            module_file_index_by_identity: data.module_file_index_by_identity.clone(),
            module_scope_by_file: data.module_scope_by_file.clone(),
            module_local_values_by_file: data.module_local_values_by_file.clone(),
            jsx_intrinsic_elements_declarer: data.jsx_intrinsic_elements_declarer.clone(),
            type_parameter_scopes: data.type_parameter_scopes.clone(),
            type_parameter_constraint_scopes: data.type_parameter_constraint_scopes.clone(),
            timings: data.timings.clone(),
            namespace_member_resolution_depth: 0,
            cross_file_resolution_depth: 0,
            namespace_member_prefix_stack: Vec::new(),
            lowest_cycle_target_index: usize::MAX,
            structural_resolution_frames: Vec::new(),
            file_kinds: data.file_kinds.clone(),
            module_value_fallback: data.module_value_fallback.clone(),
        }
    }

    /// End-of-run sizes of the shared program type caches, sampled before
    /// [`Self::clear_program_type_caches`] tears them down.
    pub(crate) fn program_cache_stats(&self) -> crate::metrics::ProgramCacheStats {
        let (generic_type_buckets, generic_type_entries) = self
            .program_resolved_generic_types
            .lock()
            .map(|cache| {
                (
                    cache.len() as u64,
                    cache.values().map(|bucket| bucket.len() as u64).sum(),
                )
            })
            .unwrap_or_default();
        let (instantiation_buckets, instantiation_entries) = self
            .program_instantiations
            .lock()
            .map(|cache| {
                (
                    cache.len() as u64,
                    cache.values().map(|bucket| bucket.len() as u64).sum(),
                )
            })
            .unwrap_or_default();
        let physical_interface_entries = self
            .physical_interface_instantiations
            .lock()
            .map(|cache| cache.len() as u64)
            .unwrap_or_default();
        crate::metrics::ProgramCacheStats {
            generic_type_buckets,
            generic_type_entries,
            instantiation_buckets,
            instantiation_entries,
            physical_interface_entries,
        }
    }

    pub(crate) fn clear_program_type_caches(&self) {
        if let Ok(mut cache) = self.resolved_named_types.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.program_resolved_generic_types.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.program_instantiations.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.physical_interface_instantiations.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.physical_interface_declaration_templates.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.physical_interface_method_instantiations.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.physical_interface_overload_instantiations.lock() {
            cache.clear();
        }
        self.substitution_store.clear();
        if let Ok(mut environments) = self.declaration_environment_store.entries.lock() {
            environments.by_key.clear();
            environments.by_id.clear();
        }
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

    /// Registers only the *constraints* of `type_parameters` (an empty value
    /// scope is pushed alongside to keep the stacks aligned). Used while
    /// resolving a generic interface/alias body so `Internals["def"]` with
    /// `Internals extends $ZodTypeInternals<…>` is recognised as
    /// constraint-validated instead of cascading into TS2536 — without marking
    /// the resolution non-concrete (an entry in `type_parameter_scopes` would
    /// disable instantiation interning for everything resolved inside).
    pub(crate) fn push_type_parameter_constraints_only(
        &mut self,
        type_parameters: &[ParsedTypeParameter],
    ) {
        let mut constraint_scope = HashMap::new();
        for type_parameter in type_parameters {
            if let Some(constraint) = type_parameter.constraint.clone() {
                constraint_scope.insert(type_parameter.name.clone(), constraint);
            }
        }
        self.type_parameter_scopes.push(HashMap::new());
        self.type_parameter_constraint_scopes.push(constraint_scope);
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
        if self.file_name != file_name {
            self.declaration_environment_generation =
                self.declaration_environment_generation.wrapping_add(1);
            self.environment_visit_counter = self.environment_visit_counter.wrapping_add(1);
        }
        self.current_file_kind = self
            .file_kinds
            .get(&file_name)
            .copied()
            .unwrap_or(FileKind::RootSource);
        self.file_name = file_name;
    }

    /// Retained-capacity bound for the per-file utility-key overlay. A typical
    /// file records at most a handful of keys; one pathological file must not
    /// pin a huge table on the worker for the rest of the run.
    const UTILITY_KEY_OVERLAY_RETAINED_CAPACITY: usize = 1024;

    /// File-region reset for a worker context that is reused across files.
    /// Serial checking clones a fresh context per file, so each file starts
    /// from the pre-check key set; a parallel worker reuses one context, and
    /// without this reset its utility keys accumulate every checked file's
    /// entries for the worker's lifetime (and can suppress diagnostics serial
    /// checking emits). The first call moves the inherited keys into the
    /// shared baseline; later calls clear only the per-file overlay, so the
    /// reused context behaves byte-identically to a fresh clone without
    /// cloning the key set per file.
    /// `resolved_named_types` must be a fresh map rather than cleared in place:
    /// resolutions depend on the consumer file's environment, and retained
    /// declaration environments may still refer to the previous file's map.
    pub(crate) fn begin_file_check(&mut self, file_name: String) {
        self.set_file_name(file_name);
        self.type_declaration_scope = None;
        if self.utility_diagnostic_keys_baseline.is_none() {
            self.utility_diagnostic_keys_baseline =
                Some(Arc::new(std::mem::take(&mut self.utility_diagnostic_keys)));
        } else if self.utility_diagnostic_keys.capacity()
            > Self::UTILITY_KEY_OVERLAY_RETAINED_CAPACITY
        {
            self.utility_diagnostic_keys = HashSet::default();
        } else {
            self.utility_diagnostic_keys.clear();
        }
        self.diagnostic_keys.clear();
        self.diagnostic_keys_len = 0;
        debug_assert!(
            self.diagnostics.is_empty(),
            "begin_file_check: previous file's diagnostics were not taken"
        );
        self.replace_resolved_named_types(0);
    }

    /// Empties both the overlay and the baseline — the equivalent of clearing
    /// the whole pre-region key set. Used by the per-file signature-collection
    /// context, which intentionally re-reports keys recorded elsewhere.
    pub(crate) fn reset_utility_diagnostic_keys(&mut self) {
        self.utility_diagnostic_keys.clear();
        self.utility_diagnostic_keys_baseline = None;
    }

    pub(crate) fn set_symbols(&mut self, symbols: SymbolTable) {
        self.symbols = symbols;
        self.declaration_environment_generation =
            self.declaration_environment_generation.wrapping_add(1);
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
        // A namespace member's own siblings shadow any outer/global declaration of
        // the same name: inside `namespace React` a bare `MouseEvent` is
        // `React.MouseEvent` (a generic interface), not the non-generic DOM global.
        // Resolving the qualified candidates first is what keeps generic React event
        // types (`MouseEventHandler<T> = EventHandler<MouseEvent<T>>`) from degrading
        // to the arity-0 global and losing their function shape.
        for candidate in self.namespace_qualified_candidates(name) {
            if let Some(declaration) = self.lookup_type_declaration_exact(&candidate) {
                return Some(declaration);
            }
        }
        self.lookup_type_declaration_exact(name)
    }

    /// Whether name lookups for the file currently being resolved must ignore
    /// the consumer module's own declaration table (`self.type_declarations`).
    ///
    /// While a dependency `.d.ts` declaration body is being expanded from
    /// another file (a `with_file_name` frame whose file differs from the one
    /// being analyzed), `self.type_declarations` still holds the *consuming*
    /// module's local table. Consulting it would let a consumer-local type
    /// name shadow the dependency's own lexical scope inside the dependency's
    /// body — wrong per tsc, and it makes expansions depend on which module
    /// triggered them (defeating cross-module expansion reuse). The dependency
    /// file's own declarations travel in its installed resolution scope, so
    /// scope lookup serves the body's own names. Same-file resolution and
    /// windows with no installed scope keep the local-table consult.
    fn lookup_ignores_local_table(&self) -> bool {
        self.cross_file_resolution_depth > 0
            && self.current_file_kind == FileKind::DependencyDeclaration
            && self.type_declaration_scope.is_some()
    }

    fn lookup_type_declaration_exact(&self, name: &str) -> Option<&TypeDeclarationInfo> {
        if !self.lookup_ignores_local_table()
            && let Some(declaration) = self.type_declarations.get(name)
        {
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
        // Namespace siblings shadow outer/global declarations of the same name — see
        // [`lookup_type_declaration`] for why this ordering matters.
        for candidate in self.namespace_qualified_candidates(name) {
            if let Some(handle) = self.lookup_type_declaration_handle_exact(&candidate) {
                return Some(handle);
            }
        }
        self.lookup_type_declaration_handle_exact(name)
    }

    fn lookup_type_declaration_handle_exact(
        &self,
        name: &str,
    ) -> Option<crate::symbols::TypeDeclarationHandle> {
        if !self.lookup_ignores_local_table()
            && let Some(handle) = self.type_declarations.get_handle(name)
        {
            crate::program::record_type_declaration_lookup(1);
            return Some(handle);
        }

        if let Some(scope) = self.type_declaration_scope.as_ref() {
            if let Some(handle) = scope.get_handle(name) {
                crate::program::record_type_declaration_lookup(2);
                return Some(handle);
            }
        }

        // The active `type_declaration_scope` can be incomplete — a declaration's
        // pre-attached `resolution_scope` may carry only its local layer (no
        // imports), and a lazy reference re-expanded outside its originating frame
        // may carry no scope at all. The authoritative per-file scope (local
        // declarations + resolved imports) lives in `module_scope_by_file`, keyed
        // by the file currently being resolved (`with_file_name` tracks it through
        // declaration bodies). Consulting it on a miss makes a type referenced from
        // file X resolvable whenever X can legitimately see it, independent of which
        // partial scope happened to be installed. This is what stabilizes mutually
        // imported clusters (ky's `Options`/`Hooks`/`NormalizedOptions` across the
        // circular `options.ts`/`hooks.ts` imports), whose resolution order would
        // otherwise leave a member degraded to `unknown`.
        crate::program::record_scope_fallback_consult();
        if let Some(scope) = self.module_scope_by_file.get(self.file_name.as_str()) {
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
        module_file_index_by_identity: FxHashMap<Arc<str>, usize>,
    ) {
        self.module_file_index_by_identity = Arc::new(module_file_index_by_identity);
        self.declaration_environment_generation =
            self.declaration_environment_generation.wrapping_add(1);
    }

    pub(crate) fn set_module_scope_by_file(
        &mut self,
        module_scope_by_file: FxHashMap<Arc<str>, Arc<TypeDeclarationScope>>,
    ) {
        self.module_scope_by_file = Arc::new(module_scope_by_file);
        self.declaration_environment_generation =
            self.declaration_environment_generation.wrapping_add(1);
    }

    /// The resolution scope of the module that declared `file_name`, used as a
    /// fallback when a declaration's pre-attached `resolution_scope` was dropped
    /// across the cyclic-import binding fixpoint. See [`Self::module_scope_by_file`].
    pub(crate) fn module_scope_for_file(
        &self,
        file_name: &str,
    ) -> Option<Arc<TypeDeclarationScope>> {
        crate::program::record_scope_fallback_consult();
        self.module_scope_by_file.get(file_name).cloned()
    }

    pub(crate) fn set_module_local_values_by_file(
        &mut self,
        module_local_values_by_file: FxHashMap<Arc<str>, Arc<SymbolTable>>,
    ) {
        self.module_local_values_by_file = Arc::new(module_local_values_by_file);
        self.declaration_environment_generation =
            self.declaration_environment_generation.wrapping_add(1);
    }

    /// The local value symbols of the module that declared `file_name`, used to
    /// resolve a `typeof <localValue>` inside an imported declaration's body
    /// (resolved under the declaring file's name via `with_file_name`, but against
    /// the consumer's value `symbols`). See [`Self::module_local_values_by_file`].
    pub(crate) fn module_local_values_for_file(&self, file_name: &str) -> Option<Arc<SymbolTable>> {
        self.module_local_values_by_file.get(file_name).cloned()
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        if self.should_suppress(&diagnostic) {
            self.record_suppressed(&diagnostic);
            return;
        }
        self.push_deduplicated(diagnostic);
    }

    /// Merges a diagnostic that already passed suppression in the context it
    /// was emitted from (a parallel analysis worker's, whose `current_file_kind`
    /// matched the emitting module). Must not re-run [`Self::should_suppress`]:
    /// this context's current file is unrelated to the diagnostic's origin at
    /// merge time, only the order-preserving dedup applies.
    pub(crate) fn push_collected(&mut self, diagnostic: Diagnostic) {
        self.push_deduplicated(diagnostic);
    }

    fn push_deduplicated(&mut self, diagnostic: Diagnostic) {
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

        if !self
            .diagnostic_keys
            .insert(Self::diagnostic_dedup_key(&diagnostic))
        {
            return;
        }

        self.diagnostics.push(diagnostic);
        self.diagnostic_keys_len = self.diagnostics.len();
    }

    fn diagnostic_dedup_key(
        diagnostic: &Diagnostic,
    ) -> (
        String,
        String,
        String,
        Option<surge_ts_diagnostics::TextSpan>,
    ) {
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

        if self
            .utility_diagnostic_keys_baseline
            .as_ref()
            .is_some_and(|baseline| baseline.contains(&key))
        {
            return;
        }
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
