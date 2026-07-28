//! Serial-equivalent speculative check sessions.
//!
//! The six program-wide resolution caches (`program_resolved_generic_types`,
//! `program_instantiations`, and the four `physical_interface_*` caches) are
//! order-visible: whether a file's resolution *hits* an entry seeded by an
//! earlier-checked file can change a rendered type display (nominal
//! `ReadonlyArray<Auth>` vs structural `Auth[]`), so racing parallel workers
//! against the shared maps changes output bytes between runs. Normalizing the
//! hit/miss display forms is forbidden — a prior attempt changed real
//! diagnostics (see `docs/perf/SPECULATIVE-TRANSACTIONAL-CHECKING.md` §3).
//!
//! This module instead makes parallel checking *serial-equivalent*: workers
//! never write the live caches. Each worker checks files against an immutable
//! snapshot of the caches taken at fan-out plus a private overlay of its own
//! insertions, and records per file which cache keys it observed *missing*.
//! After the workers finish, a single-threaded coordinator commits files in
//! serial file order: a file whose miss-set is disjoint from everything
//! published by earlier-ordered files behaved exactly as it would have under
//! serial checking, so its speculative result and cache insertions are
//! published as-is. A file that missed a key an earlier file published (or
//! that consumed an overlay entry from a file that itself failed validation)
//! may have observed a hit/miss pattern serial checking would not produce; its
//! speculative result is discarded and the file is re-checked serially against
//! the now-committed cache state. By induction over file order the committed
//! results and final cache contents are byte-identical to a serial run.
//!
//! Conflict keys are equality-consistent structural digests
//! ([`surge_ts_types::type_conflict_digest`]); a digest collision only causes
//! a spurious (sound) recheck, never a missed conflict.
//!
//! The session is installed per worker *thread* (the `ACTIVE_TYPE_STORE`
//! pattern) rather than plumbed through `CheckerContext`, because resolution
//! can materialize shadow contexts from captured declaration environments
//! mid-check ([`crate::context::DeclarationEnvironmentHandle::checker_context`]);
//! those clones carry the same live cache `Arc`s and must route through the
//! same overlay. Contexts with deliberately isolated cache handles (e.g. the
//! export-value shadow context) are recognized by pointer identity and keep
//! their isolated behavior.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use surge_ts_types::fx::{FxHashMap, FxHashSet, FxHasher};
use surge_ts_types::{FunctionType, Type, type_conflict_digest};

use crate::context::{
    CheckerContext, DeclarationResolutionKey, GenericInstantiationCacheEntry,
    InstantiationCacheEntry, InterfaceDeclarationTemplate, InterfaceInstantiationKey,
    InterfaceMemberInstantiationKey, InterfaceOverloadInstantiationKey,
    StableInterfaceDeclarationId,
};

type GenericMap = FxHashMap<DeclarationResolutionKey, Vec<GenericInstantiationCacheEntry>>;
type InstantiationMap = FxHashMap<DeclarationResolutionKey, Vec<InstantiationCacheEntry>>;
type PhysicalMap = FxHashMap<InterfaceInstantiationKey, Arc<Type>>;
type TemplateMap = FxHashMap<StableInterfaceDeclarationId, Arc<InterfaceDeclarationTemplate>>;
type MethodMap = FxHashMap<InterfaceMemberInstantiationKey, FunctionType>;
type OverloadMap = FxHashMap<InterfaceOverloadInstantiationKey, FunctionType>;

const TAG_GENERIC: u8 = 0;
const TAG_INSTANTIATION: u8 = 1;
const TAG_PHYSICAL: u8 = 2;
const TAG_TEMPLATE: u8 = 3;
const TAG_METHOD: u8 = 4;
const TAG_OVERLOAD: u8 = 5;

fn digest_flat<K: Hash>(tag: u8, key: &K) -> u64 {
    let mut hasher = FxHasher::default();
    tag.hash(&mut hasher);
    key.hash(&mut hasher);
    hasher.finish()
}

fn digest_bucket(tag: u8, key: &DeclarationResolutionKey, arguments: &[Type]) -> u64 {
    let mut hasher = FxHasher::default();
    tag.hash(&mut hasher);
    key.hash(&mut hasher);
    arguments.len().hash(&mut hasher);
    for argument in arguments {
        type_conflict_digest(argument).hash(&mut hasher);
    }
    hasher.finish()
}

/// The live shared cache maps, held so the session can (a) recognize contexts
/// whose handles are the live ones by pointer identity and (b) publish into
/// them at commit time.
#[derive(Clone)]
pub(crate) struct LiveCacheHandles {
    generic: Arc<Mutex<GenericMap>>,
    instantiations: Arc<Mutex<InstantiationMap>>,
    physical: Arc<Mutex<PhysicalMap>>,
    templates: Arc<Mutex<TemplateMap>>,
    methods: Arc<Mutex<MethodMap>>,
    overloads: Arc<Mutex<OverloadMap>>,
}

impl LiveCacheHandles {
    pub(crate) fn capture(ctx: &CheckerContext) -> Self {
        Self {
            generic: ctx.program_resolved_generic_types.clone(),
            instantiations: ctx.program_instantiations.clone(),
            physical: ctx.physical_interface_instantiations.clone(),
            templates: ctx.physical_interface_declaration_templates.clone(),
            methods: ctx.physical_interface_method_instantiations.clone(),
            overloads: ctx.physical_interface_overload_instantiations.clone(),
        }
    }
}

/// Immutable clones of the six caches at fan-out, shared by every worker's
/// session. The live maps are not written between fan-out and commit, so this
/// is exactly the state serial checking would start from.
pub(crate) struct CacheSnapshots {
    generic: GenericMap,
    instantiations: InstantiationMap,
    physical: PhysicalMap,
    templates: TemplateMap,
    methods: MethodMap,
    overloads: OverloadMap,
}

impl CacheSnapshots {
    pub(crate) fn capture(live: &LiveCacheHandles) -> Self {
        Self {
            generic: live.generic.lock().map(|m| m.clone()).unwrap_or_default(),
            instantiations: live
                .instantiations
                .lock()
                .map(|m| m.clone())
                .unwrap_or_default(),
            physical: live.physical.lock().map(|m| m.clone()).unwrap_or_default(),
            templates: live.templates.lock().map(|m| m.clone()).unwrap_or_default(),
            methods: live.methods.lock().map(|m| m.clone()).unwrap_or_default(),
            overloads: live.overloads.lock().map(|m| m.clone()).unwrap_or_default(),
        }
    }
}

/// Everything one file's speculative check observed and produced against the
/// six caches: the keys it saw missing, the worker-overlay entries it consumed
/// (by inserting file), and the entries it inserted. This is the transaction
/// the coordinator validates and publishes.
#[derive(Default)]
pub(crate) struct FileCacheLog {
    pub(crate) file_index: usize,
    misses: FxHashSet<u64>,
    /// Files (same worker, earlier in file order) whose overlay insertions this
    /// file's lookups hit. If any of them fails validation, this file's hits may
    /// not match serial and it must be rechecked too.
    overlay_deps: FxHashSet<usize>,
    generic_inserts: Vec<(
        DeclarationResolutionKey,
        GenericInstantiationCacheEntry,
        u64,
    )>,
    instantiation_inserts: Vec<(DeclarationResolutionKey, InstantiationCacheEntry, u64)>,
    physical_inserts: Vec<(InterfaceInstantiationKey, Arc<Type>, u64)>,
    template_inserts: Vec<(
        StableInterfaceDeclarationId,
        Arc<InterfaceDeclarationTemplate>,
        u64,
    )>,
    method_inserts: Vec<(InterfaceMemberInstantiationKey, FunctionType, u64)>,
    overload_inserts: Vec<(InterfaceOverloadInstantiationKey, FunctionType, u64)>,
}

impl FileCacheLog {
    pub(crate) fn miss_count(&self) -> usize {
        self.misses.len()
    }

    /// Reserves every key this file publishes into `table`, stamped with the
    /// file's serial position (`publisher`). Used to seed the reservation table
    /// from the worker logs before the replay walk.
    pub(crate) fn reserve_into(
        &self,
        table: &mut ReservationTable,
        publisher: usize,
        attempt: u64,
    ) {
        for (key, entry, _) in &self.generic_inserts {
            table.reserve_generic(key, &entry.arguments, publisher, attempt);
        }
        for (key, entry, _) in &self.instantiation_inserts {
            table.reserve_instantiation(key, &entry.arguments, publisher, attempt);
        }
        for (key, _, _) in &self.physical_inserts {
            table.reserve_physical(key, publisher, attempt);
        }
        for (key, _, _) in &self.template_inserts {
            table.reserve_template(key, publisher, attempt);
        }
        for (key, _, _) in &self.method_inserts {
            table.reserve_method(key, publisher, attempt);
        }
        for (key, _, _) in &self.overload_inserts {
            table.reserve_overload(key, publisher, attempt);
        }
    }

    /// Every insertion digest in this log, across all six caches. Measurement
    /// probes (`SURGE_DEFER_DIFF`) use this to diff a position's real insert
    /// set against its worker-log prediction.
    pub(crate) fn insert_digests(&self) -> impl Iterator<Item = u64> + '_ {
        self.generic_inserts
            .iter()
            .map(|(_, _, d)| *d)
            .chain(self.instantiation_inserts.iter().map(|(_, _, d)| *d))
            .chain(self.physical_inserts.iter().map(|(_, _, d)| *d))
            .chain(self.template_inserts.iter().map(|(_, _, d)| *d))
            .chain(self.method_inserts.iter().map(|(_, _, d)| *d))
            .chain(self.overload_inserts.iter().map(|(_, _, d)| *d))
    }

    pub(crate) fn miss_digests(&self) -> impl Iterator<Item = u64> + '_ {
        self.misses.iter().copied()
    }

    /// Debug probe: a stable one-line summary of this file's cache insertions
    /// (digests plus degraded flags), for regime-bisection dumps.
    pub(crate) fn debug_insert_line(&self) -> String {
        let mut generic: Vec<String> = self
            .generic_inserts
            .iter()
            .map(|(_, entry, digest)| format!("g{digest:x}:{}", u8::from(entry.had_error)))
            .collect();
        generic.sort_unstable();
        let mut other: Vec<String> = self
            .instantiation_inserts
            .iter()
            .map(|(_, _, digest)| format!("i{digest:x}"))
            .chain(
                self.physical_inserts
                    .iter()
                    .map(|(_, _, d)| format!("p{d:x}")),
            )
            .chain(
                self.template_inserts
                    .iter()
                    .map(|(_, _, d)| format!("t{d:x}")),
            )
            .chain(
                self.method_inserts
                    .iter()
                    .map(|(_, _, d)| format!("m{d:x}")),
            )
            .chain(
                self.overload_inserts
                    .iter()
                    .map(|(_, _, d)| format!("o{d:x}")),
            )
            .collect();
        other.sort_unstable();
        format!(
            "misses={} {} {}",
            self.misses.len(),
            generic.join(","),
            other.join(",")
        )
    }

    /// Debug probe: the file's observed-miss digests, for regime diffing.
    pub(crate) fn debug_miss_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .misses
            .iter()
            .map(|digest| format!("x{digest:x}"))
            .collect();
        lines.sort_unstable();
        lines
    }

    /// Debug probe: one line per insertion with a display-sensitive value
    /// fingerprint, for regime-divergence hunts (which committed value differs
    /// between the speculative and serial computation of the same module).
    pub(crate) fn debug_value_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .generic_inserts
            .iter()
            .map(|(key, entry, digest)| {
                let mut key_debug = format!("{key:?}");
                key_debug.truncate(220);
                format!(
                    "g{digest:x}=v{:x} key={key_debug}",
                    display_type_fingerprint(&entry.ty)
                )
            })
            .chain(
                self.instantiation_inserts
                    .iter()
                    .map(|(key, entry, digest)| {
                        let mut key_debug = format!("{key:?}");
                        key_debug.truncate(220);
                        format!(
                            "i{digest:x}=v{:x} key={key_debug}",
                            display_type_fingerprint(&entry.resolved)
                        )
                    }),
            )
            .chain(self.physical_inserts.iter().map(|(_, resolved, digest)| {
                format!("p{digest:x}=v{:x}", display_type_fingerprint(resolved))
            }))
            .chain(self.method_inserts.iter().map(|(_, function, digest)| {
                format!("m{digest:x}=v{:x}", display_function_fingerprint(function))
            }))
            .chain(self.overload_inserts.iter().map(|(_, function, digest)| {
                format!("o{digest:x}=v{:x}", display_function_fingerprint(function))
            }))
            .collect();
        lines.sort_unstable();
        lines
    }
}

/// Display-sensitive, sharing-insensitive fingerprint of a type's rendered
/// prefix: hashes discriminants, literals, reference display names, object
/// alias/property names — everything diagnostic text can show — over a
/// budget-bounded tree walk (no pointer memo, so two equal-display values hash
/// equal regardless of internal Arc sharing).
pub(crate) fn display_type_fingerprint(ty: &Type) -> u64 {
    let mut hasher = FxHasher::default();
    let mut budget = 500_000usize;
    display_fingerprint_walk(ty, &mut hasher, &mut budget);
    hasher.finish()
}

pub(crate) fn display_function_fingerprint(function: &FunctionType) -> u64 {
    let mut hasher = FxHasher::default();
    let mut budget = 500_000usize;
    display_fingerprint_function(function, &mut hasher, &mut budget);
    hasher.finish()
}

fn display_fingerprint_walk(ty: &Type, hasher: &mut impl Hasher, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    std::mem::discriminant(ty).hash(hasher);
    match ty {
        Type::StringLiteral(value) => value.hash(hasher),
        Type::NumberLiteral(value) => value.value.hash(hasher),
        Type::BooleanLiteral(value) => value.hash(hasher),
        Type::Array(element) => display_fingerprint_walk(element, hasher, budget),
        Type::Tuple(elements) => {
            for element in elements {
                display_fingerprint_walk(element, hasher, budget);
            }
        }
        Type::Union(union) => {
            for member in union.types() {
                display_fingerprint_walk(member, hasher, budget);
            }
        }
        Type::Function(function) => display_fingerprint_function(function, hasher, budget),
        Type::Object(object) => {
            object.alias_name.hash(hasher);
            for (name, property) in object.properties.iter() {
                name.hash(hasher);
                property.is_optional().hash(hasher);
                display_fingerprint_walk(&property.ty, hasher, budget);
                if *budget == 0 {
                    return;
                }
            }
        }
        Type::Reference(reference) => {
            reference.display.hash(hasher);
            for argument in reference.arguments.iter() {
                display_fingerprint_walk(argument, hasher, budget);
            }
        }
        _ => {}
    }
}

fn display_fingerprint_function(
    function: &FunctionType,
    hasher: &mut impl Hasher,
    budget: &mut usize,
) {
    for parameter in function.parameters() {
        display_fingerprint_walk(parameter, hasher, budget);
        if *budget == 0 {
            return;
        }
    }
    display_fingerprint_walk(function.return_type(), hasher, budget);
}

struct WorkerOverlay {
    generic: FxHashMap<DeclarationResolutionKey, Vec<(usize, GenericInstantiationCacheEntry)>>,
    instantiations: FxHashMap<DeclarationResolutionKey, Vec<(usize, InstantiationCacheEntry)>>,
    physical: FxHashMap<InterfaceInstantiationKey, (usize, Arc<Type>)>,
    templates: FxHashMap<StableInterfaceDeclarationId, (usize, Arc<InterfaceDeclarationTemplate>)>,
    methods: FxHashMap<InterfaceMemberInstantiationKey, (usize, FunctionType)>,
    overloads: FxHashMap<InterfaceOverloadInstantiationKey, (usize, FunctionType)>,
    /// Instantiation digests this attempt has already deferred once. A deferred
    /// key must defer at most once per attempt: the nominal reference the peel
    /// returns re-enters resolution when it is later forced (`Type::peeled` is
    /// `reference.resolve().peeled()`), and a second deferral would hand back
    /// another deferring nominal, so peeling would recurse forever. On the
    /// second lookup the key resolves as a normal miss (expands to a concrete
    /// type), terminating the peel. The attempt is discarded and requeued
    /// regardless, so this only bounds work, never changes the committed result.
    deferred_once: FxHashSet<u64>,
    current: FileCacheLog,
    current_active: bool,
    finished: Vec<FileCacheLog>,
}

impl WorkerOverlay {
    fn new() -> Self {
        Self {
            generic: FxHashMap::default(),
            instantiations: FxHashMap::default(),
            physical: FxHashMap::default(),
            templates: FxHashMap::default(),
            methods: FxHashMap::default(),
            overloads: FxHashMap::default(),
            deferred_once: FxHashSet::default(),
            current: FileCacheLog::default(),
            current_active: false,
            finished: Vec::new(),
        }
    }

    fn record_dep(&mut self, inserting_file: usize) {
        if inserting_file != self.current.file_index {
            self.current.overlay_deps.insert(inserting_file);
        }
    }
}

/// The committed cache state a session reads beneath its overlay. Workers use
/// an immutable [`CacheSnapshots`] taken at fan-out (lock-free, shared by all
/// workers); coordinator rechecks read the live maps directly — the commit
/// pass is single-threaded, so the live maps are exactly the committed state
/// and cloning a fresh snapshot per recheck would dominate commit time.
enum BaseView {
    Snapshot(Arc<CacheSnapshots>),
    Live,
}

/// Measurement-only deferral context (Stage 2). When a replay session reads the
/// live committed store and misses a key, it consults the reservation table for
/// this position: a `Deferred` result means an earlier not-yet-committed
/// position would have published the key, so serial checking would have hit it
/// and the replay is about to over-recurse. This context only *counts* those
/// events (behind `SURGE_DEFER_MEASURE`); it never changes resolution, so output
/// stays byte-identical. It quantifies the deferral opportunity before any
/// abort-and-requeue mechanism is built.
struct DeferralContext {
    table: Arc<std::sync::RwLock<ReservationTable>>,
    position: usize,
    stats: Arc<DeferralStats>,
    /// Latest (largest) blocking publisher this attempt deferred to, or -1 if it
    /// never deferred. The requeue waits on the latest so the re-run reads the
    /// most-committed view; `-1` means the replay ran to completion (valid).
    max_deferred: std::sync::atomic::AtomicI64,
}

impl DeferralContext {
    /// Queries the reservation table for this position, records the deferral in
    /// the stats and the attempt's `max_deferred`, and returns the blocking
    /// publisher if the key is owned by an earlier not-yet-committed position.
    fn check(
        &self,
        query: impl FnOnce(&ReservationTable, usize) -> ReservationLookup,
    ) -> Option<usize> {
        use std::sync::atomic::Ordering::Relaxed;
        self.stats.queried.fetch_add(1, Relaxed);
        if let Ok(table) = self.table.read()
            && let ReservationLookup::Deferred { publisher, .. } = query(&table, self.position)
        {
            self.stats.deferred.fetch_add(1, Relaxed);
            self.max_deferred.fetch_max(publisher as i64, Relaxed);
            return Some(publisher);
        }
        None
    }
}

/// Outcome of a deferral-aware instantiation probe. `Deferred` is control flow —
/// never a `Type` — converted to a nominal reference only at the lazy-peel
/// boundary; every other caller treats it as a miss.
pub(crate) enum InstantiationProbe {
    Hit(InstantiationCacheEntry),
    Miss,
    Deferred,
}

/// Atomic counters shared across the replay pool for the deferral measurement.
#[derive(Default)]
pub(crate) struct DeferralStats {
    pub(crate) queried: std::sync::atomic::AtomicU64,
    pub(crate) deferred: std::sync::atomic::AtomicU64,
}

/// One worker's speculative view of the six program caches.
pub(crate) struct CheckSession {
    live: LiveCacheHandles,
    base: BaseView,
    state: Mutex<WorkerOverlay>,
    defer: Option<DeferralContext>,
}

impl CheckSession {
    pub(crate) fn new(live: LiveCacheHandles, base: Arc<CacheSnapshots>) -> Self {
        Self {
            live,
            base: BaseView::Snapshot(base),
            state: Mutex::new(WorkerOverlay::new()),
            defer: None,
        }
    }

    /// A session whose base reads go straight to the live maps. Only sound
    /// while nothing else writes them — i.e. during the single-threaded commit
    /// pass's rechecks.
    pub(crate) fn new_live_reading(live: LiveCacheHandles) -> Self {
        Self {
            live,
            base: BaseView::Live,
            state: Mutex::new(WorkerOverlay::new()),
            defer: None,
        }
    }

    /// A live-reading session that additionally records (measurement only) how
    /// often a miss would defer to an earlier pending publisher at `position`.
    pub(crate) fn new_live_reading_deferring(
        live: LiveCacheHandles,
        table: Arc<std::sync::RwLock<ReservationTable>>,
        position: usize,
        stats: Arc<DeferralStats>,
    ) -> Self {
        Self {
            live,
            base: BaseView::Live,
            state: Mutex::new(WorkerOverlay::new()),
            defer: Some(DeferralContext {
                table,
                position,
                stats,
                max_deferred: std::sync::atomic::AtomicI64::new(-1),
            }),
        }
    }

    /// The blocking publisher this replay deferred to (the latest), or `None` if
    /// it ran to completion without deferring. Used by the replay pipeline to
    /// requeue a deferred attempt once that publisher commits.
    pub(crate) fn deferred_until(&self) -> Option<usize> {
        self.defer.as_ref().and_then(|defer| {
            let value = defer
                .max_deferred
                .load(std::sync::atomic::Ordering::Relaxed);
            (value >= 0).then_some(value as usize)
        })
    }

    fn base_generic_hit(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> Option<(Type, bool)> {
        let find = |bucket: &[GenericInstantiationCacheEntry]| {
            bucket
                .iter()
                .find(|entry| entry.arguments == arguments)
                .map(|entry| (entry.ty.clone(), entry.had_error))
        };
        match &self.base {
            BaseView::Snapshot(base) => base.generic.get(key).and_then(|bucket| find(bucket)),
            BaseView::Live => self
                .live
                .generic
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).and_then(|bucket| find(bucket))),
        }
    }

    /// Returns whether the base bucket already holds `arguments` and the
    /// bucket's current length (for the insertion cap).
    fn base_generic_probe(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> (bool, usize) {
        let probe = |bucket: &[GenericInstantiationCacheEntry]| {
            (
                bucket.iter().any(|entry| entry.arguments == arguments),
                bucket.len(),
            )
        };
        match &self.base {
            BaseView::Snapshot(base) => base.generic.get(key).map_or((false, 0), |b| probe(b)),
            BaseView::Live => self.live.generic.lock().ok().map_or((false, 0), |cache| {
                cache.get(key).map_or((false, 0), |b| probe(b))
            }),
        }
    }

    fn base_instantiation_entry(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> Option<InstantiationCacheEntry> {
        let find = |bucket: &[InstantiationCacheEntry]| {
            bucket
                .iter()
                .find(|entry| entry.arguments == arguments)
                .cloned()
        };
        match &self.base {
            BaseView::Snapshot(base) => base.instantiations.get(key).and_then(|b| find(b)),
            BaseView::Live => self
                .live
                .instantiations
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).and_then(|b| find(b))),
        }
    }

    fn base_instantiation_len(&self, key: &DeclarationResolutionKey) -> usize {
        match &self.base {
            BaseView::Snapshot(base) => base.instantiations.get(key).map_or(0, Vec::len),
            BaseView::Live => self
                .live
                .instantiations
                .lock()
                .ok()
                .map_or(0, |cache| cache.get(key).map_or(0, Vec::len)),
        }
    }

    fn base_physical(&self, key: &InterfaceInstantiationKey) -> Option<Arc<Type>> {
        match &self.base {
            BaseView::Snapshot(base) => base.physical.get(key).cloned(),
            BaseView::Live => self
                .live
                .physical
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).cloned()),
        }
    }

    fn base_template(
        &self,
        key: &StableInterfaceDeclarationId,
    ) -> Option<Arc<InterfaceDeclarationTemplate>> {
        match &self.base {
            BaseView::Snapshot(base) => base.templates.get(key).cloned(),
            BaseView::Live => self
                .live
                .templates
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).cloned()),
        }
    }

    fn base_method(&self, key: &InterfaceMemberInstantiationKey) -> Option<FunctionType> {
        match &self.base {
            BaseView::Snapshot(base) => base.methods.get(key).cloned(),
            BaseView::Live => self
                .live
                .methods
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).cloned()),
        }
    }

    fn base_overload(&self, key: &InterfaceOverloadInstantiationKey) -> Option<FunctionType> {
        match &self.base {
            BaseView::Snapshot(base) => base.overloads.get(key).cloned(),
            BaseView::Live => self
                .live
                .overloads
                .lock()
                .ok()
                .and_then(|cache| cache.get(key).cloned()),
        }
    }

    /// Starts recording a new file's observations. Files checked by one worker
    /// arrive in ascending file order (the dispatch counter is monotonic), so
    /// the worker overlay only ever contains entries from files earlier in
    /// serial order than the current one.
    pub(crate) fn begin_file(&self, file_index: usize) {
        let mut state = self.state.lock().expect("check session poisoned");
        if state.current_active {
            let finished = std::mem::take(&mut state.current);
            state.finished.push(finished);
        }
        state.current = FileCacheLog {
            file_index,
            ..FileCacheLog::default()
        };
        state.current_active = true;
    }

    /// Flushes the in-progress file log and returns every file log this worker
    /// produced, in the worker's (ascending) check order.
    pub(crate) fn take_file_logs(&self) -> Vec<FileCacheLog> {
        let mut state = self.state.lock().expect("check session poisoned");
        if state.current_active {
            let finished = std::mem::take(&mut state.current);
            state.finished.push(finished);
            state.current_active = false;
        }
        std::mem::take(&mut state.finished)
    }

    pub(crate) fn owns_generic(&self, handle: &Arc<Mutex<GenericMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.generic)
    }

    pub(crate) fn owns_instantiations(&self, handle: &Arc<Mutex<InstantiationMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.instantiations)
    }

    pub(crate) fn owns_physical(&self, handle: &Arc<Mutex<PhysicalMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.physical)
    }

    pub(crate) fn owns_templates(&self, handle: &Arc<Mutex<TemplateMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.templates)
    }

    pub(crate) fn owns_methods(&self, handle: &Arc<Mutex<MethodMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.methods)
    }

    pub(crate) fn owns_overloads(&self, handle: &Arc<Mutex<OverloadMap>>) -> bool {
        Arc::ptr_eq(handle, &self.live.overloads)
    }

    /// Mirrors `get_persistent_generic_resolution`: snapshot bucket first (the
    /// entries serial checking would have started with, in their original
    /// order), then the worker overlay (this worker's own earlier appends).
    pub(crate) fn generic_lookup(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> Option<(Type, bool)> {
        if let Some(hit) = self.base_generic_hit(key, arguments) {
            return Some(hit);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, entry)) = state.generic.get(key).and_then(|bucket| {
            bucket
                .iter()
                .find(|(_, entry)| entry.arguments == arguments)
                .cloned()
        }) {
            state.record_dep(file);
            return Some((entry.ty.clone(), entry.had_error));
        }
        let digest = digest_bucket(TAG_GENERIC, key, arguments);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer {
            defer.check(|table, k| table.query_generic(key, arguments, k));
        }
        None
    }

    /// Mirrors `cache_persistent_generic_resolution` (probe, cap, insert).
    pub(crate) fn generic_insert(
        &self,
        key: &DeclarationResolutionKey,
        arguments: Vec<Type>,
        ty: Type,
        had_error: bool,
        cap: usize,
    ) {
        let (base_exists, base_len) = self.base_generic_probe(key, &arguments);
        if base_exists {
            return;
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some(bucket) = state.generic.get(key)
            && bucket.iter().any(|(_, entry)| entry.arguments == arguments)
        {
            return;
        }
        let digest = digest_bucket(TAG_GENERIC, key, &arguments);
        state.current.misses.insert(digest);
        let overlay_len = state.generic.get(key).map_or(0, Vec::len);
        if base_len + overlay_len >= cap {
            crate::program::record_program_counter(|c| c.generic_type_cache_capped_count += 1);
            return;
        }
        crate::program::record_program_counter(|c| c.generic_type_cache_insert_count += 1);
        let entry = GenericInstantiationCacheEntry {
            arguments,
            ty,
            had_error,
        };
        let file_index = state.current.file_index;
        state
            .current
            .generic_inserts
            .push((key.clone(), entry.clone(), digest));
        state
            .generic
            .entry(key.clone())
            .or_default()
            .push((file_index, entry));
    }

    /// Mirrors `intern_instantiation` (hit returns the shared expansion, miss
    /// interns under the bucket cap).
    pub(crate) fn instantiation_intern(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        structural: Type,
        cap: usize,
    ) -> Arc<Type> {
        if let Some(entry) = self.base_instantiation_entry(key, arguments) {
            crate::program::record_program_counter(|c| c.instantiation_intern_hit_count += 1);
            return entry.resolved;
        }
        let base_len = self.base_instantiation_len(key);
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, resolved)) = state.instantiations.get(key).and_then(|bucket| {
            bucket
                .iter()
                .find(|(_, entry)| entry.arguments == arguments)
                .map(|(file, entry)| (*file, entry.resolved.clone()))
        }) {
            state.record_dep(file);
            crate::program::record_program_counter(|c| c.instantiation_intern_hit_count += 1);
            return resolved;
        }
        let digest = digest_bucket(TAG_INSTANTIATION, key, arguments);
        state.current.misses.insert(digest);
        let resolved = Arc::new(structural);
        let overlay_len = state.instantiations.get(key).map_or(0, Vec::len);
        if base_len + overlay_len < cap {
            crate::program::record_program_counter(|c| c.instantiation_intern_insert_count += 1);
            let entry = InstantiationCacheEntry {
                arguments: arguments.to_vec(),
                resolved: resolved.clone(),
            };
            let file_index = state.current.file_index;
            state
                .current
                .instantiation_inserts
                .push((key.clone(), entry.clone(), digest));
            state
                .instantiations
                .entry(key.clone())
                .or_default()
                .push((file_index, entry));
        } else {
            crate::program::record_program_counter(|c| c.instantiation_intern_capped_count += 1);
        }
        resolved
    }

    /// Mirrors `lookup_instantiation`.
    pub(crate) fn instantiation_lookup(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> Option<InstantiationCacheEntry> {
        match self.instantiation_probe(key, arguments) {
            InstantiationProbe::Hit(entry) => Some(entry),
            InstantiationProbe::Miss | InstantiationProbe::Deferred => None,
        }
    }

    /// Deferral-aware instantiation lookup used by the lazy peel: a miss whose
    /// key is owned by an earlier not-yet-committed publisher returns `Deferred`
    /// (at most once per key per attempt — see `WorkerOverlay::deferred_once`)
    /// instead of `Miss`, so the peel can return the nominal form rather than
    /// over-recursing into a declaration the earlier position will publish.
    pub(crate) fn instantiation_probe(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
    ) -> InstantiationProbe {
        if let Some(entry) = self.base_instantiation_entry(key, arguments) {
            return InstantiationProbe::Hit(entry);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, entry)) = state.instantiations.get(key).and_then(|bucket| {
            bucket
                .iter()
                .find(|(_, entry)| entry.arguments == arguments)
                .cloned()
        }) {
            state.record_dep(file);
            return InstantiationProbe::Hit(entry);
        }
        let digest = digest_bucket(TAG_INSTANTIATION, key, arguments);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer
            && !state.deferred_once.contains(&digest)
            && defer
                .check(|table, k| table.query_instantiation(key, arguments, k))
                .is_some()
        {
            state.deferred_once.insert(digest);
            return InstantiationProbe::Deferred;
        }
        InstantiationProbe::Miss
    }

    pub(crate) fn physical_lookup(&self, key: &InterfaceInstantiationKey) -> Option<Arc<Type>> {
        if let Some(resolved) = self.base_physical(key) {
            return Some(resolved);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, resolved)) = state.physical.get(key).cloned() {
            state.record_dep(file);
            return Some(resolved);
        }
        let digest = digest_flat(TAG_PHYSICAL, key);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer {
            defer.check(|table, k| table.query_physical(key, k));
        }
        None
    }

    pub(crate) fn physical_intern(
        &self,
        key: InterfaceInstantiationKey,
        resolved: Type,
    ) -> Arc<Type> {
        if let Some(existing) = self.base_physical(&key) {
            crate::program::record_program_counter(|c| {
                c.physical_interface_cache_racing_insert_count += 1
            });
            return existing;
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, existing)) = state.physical.get(&key).cloned() {
            state.record_dep(file);
            crate::program::record_program_counter(|c| {
                c.physical_interface_cache_racing_insert_count += 1
            });
            return existing;
        }
        let digest = digest_flat(TAG_PHYSICAL, &key);
        state.current.misses.insert(digest);
        let resolved = Arc::new(resolved);
        crate::program::record_program_counter(|c| c.physical_interface_cache_insert_count += 1);
        let file_index = state.current.file_index;
        state
            .current
            .physical_inserts
            .push((key.clone(), resolved.clone(), digest));
        state.physical.insert(key, (file_index, resolved.clone()));
        resolved
    }

    pub(crate) fn template_lookup(
        &self,
        key: &StableInterfaceDeclarationId,
    ) -> Option<Arc<InterfaceDeclarationTemplate>> {
        if let Some(template) = self.base_template(key) {
            return Some(template);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, template)) = state.templates.get(key).cloned() {
            state.record_dep(file);
            return Some(template);
        }
        let digest = digest_flat(TAG_TEMPLATE, key);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer {
            defer.check(|table, k| table.query_template(key, k));
        }
        None
    }

    pub(crate) fn template_intern(
        &self,
        key: StableInterfaceDeclarationId,
        template: Arc<InterfaceDeclarationTemplate>,
        retained_bytes: u64,
    ) -> Arc<InterfaceDeclarationTemplate> {
        if let Some(existing) = self.base_template(&key) {
            return existing;
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, existing)) = state.templates.get(&key).cloned() {
            state.record_dep(file);
            return existing;
        }
        let digest = digest_flat(TAG_TEMPLATE, &key);
        state.current.misses.insert(digest);
        crate::program::record_program_counter(|c| {
            c.interface_template_insert_count += 1;
            c.interface_template_retained_bytes += retained_bytes;
        });
        let file_index = state.current.file_index;
        state
            .current
            .template_inserts
            .push((key.clone(), template.clone(), digest));
        state.templates.insert(key, (file_index, template.clone()));
        template
    }

    pub(crate) fn method_lookup(
        &self,
        key: &InterfaceMemberInstantiationKey,
    ) -> Option<FunctionType> {
        if let Some(function) = self.base_method(key) {
            return Some(function);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, function)) = state.methods.get(key).cloned() {
            state.record_dep(file);
            return Some(function);
        }
        let digest = digest_flat(TAG_METHOD, key);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer {
            defer.check(|table, k| table.query_method(key, k));
        }
        None
    }

    pub(crate) fn method_intern(
        &self,
        key: InterfaceMemberInstantiationKey,
        function: FunctionType,
        key_bytes: u64,
        value_bytes: u64,
    ) -> FunctionType {
        if let Some(existing) = self.base_method(&key) {
            return existing;
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, existing)) = state.methods.get(&key).cloned() {
            state.record_dep(file);
            return existing;
        }
        let digest = digest_flat(TAG_METHOD, &key);
        state.current.misses.insert(digest);
        crate::program::record_program_counter(|c| {
            c.interface_method_cache_insert_count += 1;
            c.interface_method_cache_key_bytes += key_bytes;
            c.interface_method_cache_value_shallow_bytes += value_bytes;
        });
        let file_index = state.current.file_index;
        state
            .current
            .method_inserts
            .push((key.clone(), function.clone(), digest));
        state.methods.insert(key, (file_index, function.clone()));
        function
    }

    pub(crate) fn overload_lookup(
        &self,
        key: &InterfaceOverloadInstantiationKey,
    ) -> Option<FunctionType> {
        if let Some(function) = self.base_overload(key) {
            return Some(function);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, function)) = state.overloads.get(key).cloned() {
            state.record_dep(file);
            return Some(function);
        }
        let digest = digest_flat(TAG_OVERLOAD, key);
        state.current.misses.insert(digest);
        if let Some(defer) = &self.defer {
            defer.check(|table, k| table.query_overload(key, k));
        }
        None
    }

    pub(crate) fn overload_intern(
        &self,
        key: InterfaceOverloadInstantiationKey,
        function: FunctionType,
        key_bytes: u64,
        value_bytes: u64,
    ) -> FunctionType {
        if let Some(existing) = self.base_overload(&key) {
            return existing;
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, existing)) = state.overloads.get(&key).cloned() {
            state.record_dep(file);
            return existing;
        }
        let digest = digest_flat(TAG_OVERLOAD, &key);
        state.current.misses.insert(digest);
        crate::program::record_program_counter(|c| {
            c.interface_overload_cache_insert_count += 1;
            c.interface_overload_cache_key_bytes += key_bytes;
            c.interface_overload_cache_value_shallow_bytes += value_bytes;
        });
        let file_index = state.current.file_index;
        state
            .current
            .overload_inserts
            .push((key.clone(), function.clone(), digest));
        state.overloads.insert(key, (file_index, function.clone()));
        function
    }
}

thread_local! {
    static ACTIVE_CHECK_SESSION: RefCell<Option<Arc<CheckSession>>> = const { RefCell::new(None) };
}

pub(crate) fn active_check_session() -> Option<Arc<CheckSession>> {
    ACTIVE_CHECK_SESSION.with(|slot| slot.borrow().clone())
}

/// Installs `session` as the current thread's speculative view for the duration
/// of `f`. Restores the previous session on exit (including unwinds), so a
/// worker panic cannot leak its session into unrelated work on a reused thread.
pub(crate) fn with_check_session<R>(session: Arc<CheckSession>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Arc<CheckSession>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_CHECK_SESSION.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = ACTIVE_CHECK_SESSION.with(|slot| slot.borrow_mut().replace(session));
    let _restore = Restore(previous);
    f()
}

#[derive(Debug, Default)]
pub(crate) struct StcCommitStats {
    pub(crate) files: usize,
    pub(crate) clean_commits: usize,
    pub(crate) miss_conflicts: usize,
    pub(crate) dependency_conflicts: usize,
    pub(crate) published_entries: usize,
    pub(crate) merge_skipped_existing: usize,
    pub(crate) merge_cap_blocked: usize,
}

pub(crate) enum CommitVerdict {
    Clean,
    /// The file observed a miss on a key an earlier-ordered file published; its
    /// speculative result may differ from serial and must be recomputed.
    MissConflict,
    /// The file consumed a worker-overlay entry inserted by a file that itself
    /// failed validation (or whose publication was incomplete).
    DependencyConflict,
}

/// Predicts which positions will conflict during the commit walk, as a
/// scheduling hint for pipelined replay. Over-prediction only wastes a replay
/// and under-prediction only triggers an inline recompute, so correctness never
/// depends on this — it merely decides which positions the pool pre-replays.
///
/// A position likely conflicts if a key it observed missing was inserted by an
/// earlier position, or it consumed the worker overlay of an earlier position
/// that is itself predicted to conflict. Dispatch is ascending per worker, so a
/// position's overlay producers are always earlier and already classified.
pub(crate) fn predict_conflicts(logs: &[Option<FileCacheLog>]) -> Vec<bool> {
    let mut predicted = vec![false; logs.len()];
    let mut earlier_inserts: FxHashSet<u64> = FxHashSet::default();
    for (index, log) in logs.iter().enumerate() {
        let Some(log) = log else { continue };
        let miss_conflict = !log.misses.is_disjoint(&earlier_inserts);
        let dep_conflict = log.overlay_deps.iter().any(|&file| predicted[file]);
        predicted[index] = miss_conflict || dep_conflict;
        for (_, _, digest) in &log.generic_inserts {
            earlier_inserts.insert(*digest);
        }
        for (_, _, digest) in &log.instantiation_inserts {
            earlier_inserts.insert(*digest);
        }
        for (_, _, digest) in &log.physical_inserts {
            earlier_inserts.insert(*digest);
        }
        for (_, _, digest) in &log.template_inserts {
            earlier_inserts.insert(*digest);
        }
        for (_, _, digest) in &log.method_inserts {
            earlier_inserts.insert(*digest);
        }
        for (_, _, digest) in &log.overload_inserts {
            earlier_inserts.insert(*digest);
        }
    }
    predicted
}

/// Dependency-driven submit schedule for the replay pipeline: for each
/// predicted-conflict position, the frontier index at which its replay should
/// start (`usize::MAX` for positions that are not pre-replayed).
///
/// A conflict `k` reads the committed store correctly only once every position
/// that publishes a key `k` observed missing has committed. The last such
/// publisher (by first-writer position, since first-writer-wins) is `k`'s
/// binding dependency; launching `k`'s replay the moment that publisher's
/// position finalizes means the replay reads a committed view already containing
/// all of `k`'s dependencies, so it does not over-recurse against a stale view
/// and validates on the first try. A conflict with no earlier publisher among
/// its misses is submittable from the start (index 0).
///
/// This only schedules; `commit_position` still validates and falls back to an
/// inline recompute, so an imprecise schedule costs at most a wasted replay or
/// an inline recompute, never correctness.
pub(crate) fn compute_submit_schedule(logs: &[Option<FileCacheLog>]) -> Vec<usize> {
    let predicted = predict_conflicts(logs);
    let n = logs.len();
    let mut submit_at = vec![usize::MAX; n];
    // First position (in serial order) that inserts each digest — the publisher
    // whose commit makes that key visible — and whether that publisher is itself
    // a predicted conflict.
    let mut first_writer: FxHashMap<u64, usize> = FxHashMap::default();
    for (index, log) in logs.iter().enumerate() {
        let Some(log) = log else { continue };
        if predicted[index] {
            let mut latest_dep: Option<usize> = None;
            // Pre-replay only conflicts whose every dependency is a *clean* file.
            // A replay reads the committed store rather than the worker's fan-out
            // snapshot, so its exact dependency set can differ from the worker
            // log; when a dependency is another conflict, that imprecision makes
            // the replay prone to staleness (the conflict's eventual committed
            // inserts need not match its worker log), and a stale replay
            // over-recurses expensively for nothing. Clean-dependent conflicts,
            // by contrast, depend only on files that commit deterministically and
            // early, so their replay reads a complete-enough view and validates.
            // Conflict-dependent positions fall to the inline recheck.
            let mut depends_on_conflict = false;
            for digest in &log.misses {
                if let Some(&producer) = first_writer.get(digest)
                    && producer < index
                {
                    latest_dep = Some(latest_dep.map_or(producer, |cur| cur.max(producer)));
                    if predicted[producer] {
                        depends_on_conflict = true;
                    }
                }
            }
            // A worker overlay-hit means the replay (which has no overlay) will
            // re-read that key from the committed store, so it depends on the
            // overlay producer having committed.
            for &producer in &log.overlay_deps {
                if producer < index {
                    latest_dep = Some(latest_dep.map_or(producer, |cur| cur.max(producer)));
                    if predicted[producer] {
                        depends_on_conflict = true;
                    }
                }
            }
            if !depends_on_conflict {
                // Submit after the last dependency's position finalizes (index
                // `producer + 1`); no dependency ⇒ submittable immediately.
                submit_at[index] = latest_dep.map_or(0, |producer| producer + 1);
            }
        }
        let mut record = |digest: u64| {
            first_writer.entry(digest).or_insert(index);
        };
        for (_, _, d) in &log.generic_inserts {
            record(*d);
        }
        for (_, _, d) in &log.instantiation_inserts {
            record(*d);
        }
        for (_, _, d) in &log.physical_inserts {
            record(*d);
        }
        for (_, _, d) in &log.template_inserts {
            record(*d);
        }
        for (_, _, d) in &log.method_inserts {
            record(*d);
        }
        for (_, _, d) in &log.overload_inserts {
            record(*d);
        }
    }
    submit_at
}

/// Diagnostic (`SURGE_REPLAY_DAG=1`): estimates the conflict dependency DAG's
/// parallel-round ceiling. A conflict's level is `1 + max level of an earlier
/// conflict whose published insert it misses`; the max level is the number of
/// serial rounds an idealized round-based replay would need.
pub(crate) fn report_conflict_dag(logs: &[Option<FileCacheLog>]) {
    if std::env::var_os("SURGE_REPLAY_DAG").is_none() {
        return;
    }
    let predicted = predict_conflicts(logs);
    let mut digest_level: FxHashMap<u64, u32> = FxHashMap::default();
    let mut histogram: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    let mut max_level = 0u32;
    let mut conflicts = 0u32;
    for (index, log) in logs.iter().enumerate() {
        let Some(log) = log else { continue };
        if !predicted[index] {
            continue;
        }
        conflicts += 1;
        let mut level = 1u32;
        for digest in &log.misses {
            if let Some(&producer_level) = digest_level.get(digest) {
                level = level.max(producer_level + 1);
            }
        }
        max_level = max_level.max(level);
        *histogram.entry(level).or_default() += 1;
        let mut publish = |digest: u64| {
            let entry = digest_level.entry(digest).or_insert(level);
            *entry = (*entry).max(level);
        };
        for (_, _, d) in &log.generic_inserts {
            publish(*d);
        }
        for (_, _, d) in &log.instantiation_inserts {
            publish(*d);
        }
        for (_, _, d) in &log.physical_inserts {
            publish(*d);
        }
        for (_, _, d) in &log.template_inserts {
            publish(*d);
        }
        for (_, _, d) in &log.method_inserts {
            publish(*d);
        }
        for (_, _, d) in &log.overload_inserts {
            publish(*d);
        }
    }
    eprintln!(
        "[stc-dag] conflicts={conflicts} max_level(round_ceiling)={max_level} level_hist={histogram:?}"
    );
}

/// Per predicted-conflict position, its conflict-DAG dependency positions (the
/// first-writers of keys it missed, plus its overlay producers, all `< index`).
/// Computed before the commit walk consumes the logs. Non-conflicts get an empty
/// list. Feeds [`report_critical_path`].
pub(crate) fn compute_conflict_deps(logs: &[Option<FileCacheLog>]) -> Vec<Vec<usize>> {
    let predicted = predict_conflicts(logs);
    let n = logs.len();
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut first_writer: FxHashMap<u64, usize> = FxHashMap::default();
    for (index, log) in logs.iter().enumerate() {
        let Some(log) = log else { continue };
        if predicted[index] {
            let mut set: FxHashSet<usize> = FxHashSet::default();
            for digest in &log.misses {
                if let Some(&producer) = first_writer.get(digest)
                    && producer < index
                {
                    set.insert(producer);
                }
            }
            for &producer in &log.overlay_deps {
                if producer < index {
                    set.insert(producer);
                }
            }
            deps[index] = set.into_iter().collect();
        }
        let mut record = |digest: u64| {
            first_writer.entry(digest).or_insert(index);
        };
        for (_, _, d) in &log.generic_inserts {
            record(*d);
        }
        for (_, _, d) in &log.instantiation_inserts {
            record(*d);
        }
        for (_, _, d) in &log.physical_inserts {
            record(*d);
        }
        for (_, _, d) in &log.template_inserts {
            record(*d);
        }
        for (_, _, d) in &log.method_inserts {
            record(*d);
        }
        for (_, _, d) in &log.overload_inserts {
            record(*d);
        }
    }
    deps
}

/// Diagnostic (`SURGE_CRITPATH=1`): the out-of-order-commit ceiling. Given each
/// conflict position's measured recompute time (`micros[k]`) and the conflict
/// DAG (`deps`), computes the longest *weighted* dependency chain — the critical
/// path that even a perfect topological, unbounded-parallel commit could not
/// beat — alongside the serial sum. If `critical_path ≈ serial_sum` the
/// conflicts form one long chain and out-of-order commit cannot help; if
/// `critical_path ≪ serial_sum` there is parallelism to exploit.
pub(crate) fn report_critical_path(deps: &[Vec<usize>], micros: &FxHashMap<usize, u128>) {
    if std::env::var_os("SURGE_CRITPATH").is_none() {
        return;
    }
    let n = deps.len();
    let mut finish: Vec<u128> = vec![0; n];
    let mut prev: Vec<Option<usize>> = vec![None; n];
    let mut serial_sum: u128 = 0;
    let mut critical_path: u128 = 0;
    let mut best_end: Option<usize> = None;
    let mut conflicts = 0u32;
    // Positions are already in ascending serial order and every dependency is
    // `< index`, so a single forward pass is a valid topological DP.
    for index in 0..n {
        let weight = *micros.get(&index).unwrap_or(&0);
        if deps[index].is_empty() && weight == 0 {
            continue;
        }
        conflicts += 1;
        serial_sum += weight;
        let mut dep_finish: u128 = 0;
        let mut dep_prev: Option<usize> = None;
        for &producer in &deps[index] {
            if finish[producer] > dep_finish {
                dep_finish = finish[producer];
                dep_prev = Some(producer);
            }
        }
        finish[index] = dep_finish + weight;
        prev[index] = dep_prev;
        if finish[index] > critical_path {
            critical_path = finish[index];
            best_end = Some(index);
        }
    }
    let mut chain_len = 0u32;
    let mut cursor = best_end;
    while let Some(k) = cursor {
        chain_len += 1;
        cursor = prev[k];
    }
    let ideal_8 = (serial_sum / 8).max(critical_path);
    eprintln!(
        "[stc-critpath] conflicts={conflicts} serial_sum={:.0}ms critical_path={:.0}ms \
         ideal_parallel(8core)={:.0}ms critical_chain_len={chain_len} speedup_ceiling(inf)={:.2}x \
         speedup_ceiling(8core)={:.2}x",
        serial_sum as f64 / 1000.0,
        critical_path as f64 / 1000.0,
        ideal_8 as f64 / 1000.0,
        serial_sum as f64 / critical_path.max(1) as f64,
        serial_sum as f64 / ideal_8.max(1) as f64,
    );
}

/// Validates one file's log against everything published so far. On a clean
/// verdict the file's insertions are published into the live maps (in the
/// file's own insertion order) and their digests join `published`.
///
/// Validation is by digest presence (equality-consistent structural digests).
/// A value-based refinement — commit clean when a colliding miss's value equals
/// the published value — was investigated and rejected as unsound: a worker
/// computing against the incomplete fan-out snapshot over-recurses on keys
/// serial would hit and interns spurious sub-instantiations, which pollute the
/// committed cache for later files even when the worker's own diagnostics match
/// serial (and are invisible to a value check, being new keys that never
/// collide). Only a computation reading the *complete* committed state at its
/// position avoids over-recursion — the inline recheck or a validated replay
/// (`crate::replay`). See `docs/perf/TRPC-ORDERED-DELTA-REPLAY.md`.
pub(crate) fn commit_file_log(
    live: &LiveCacheHandles,
    log: &FileCacheLog,
    published: &mut FxHashSet<u64>,
    dirty_files: &FxHashSet<usize>,
    cap: usize,
    stats: &mut StcCommitStats,
) -> CommitVerdict {
    stats.files += 1;
    if !log.misses.is_disjoint(published) {
        stats.miss_conflicts += 1;
        return CommitVerdict::MissConflict;
    }
    if log
        .overlay_deps
        .iter()
        .any(|file| dirty_files.contains(file))
    {
        stats.dependency_conflicts += 1;
        return CommitVerdict::DependencyConflict;
    }
    apply_file_log(live, log, published, cap, stats);
    stats.clean_commits += 1;
    CommitVerdict::Clean
}

/// Validates a single-file *replay* log (produced by a pool thread reading the
/// live committed store) against the published-digest set, and publishes its
/// insertions if valid. Returns whether it was applied.
///
/// A replay reads only the committed store and its own private overlay, so its
/// log never carries overlay dependencies — validation is purely
/// `misses ∩ published == ∅`. Under strict in-order publication a valid replay
/// matches the serial run at its position exactly (see `crate::replay`): a miss
/// disjoint from `published` proves the replay did not over-recurse on any key
/// serial published before this position, so its cache insertions match serial.
/// Unlike an inline recheck, a cap-blocked or already-present insertion is not
/// an error: a position between the replay's read and the frontier may have
/// grown a bucket to the cap, which serial would see identically here.
pub(crate) fn commit_replay_log(
    live: &LiveCacheHandles,
    log: &FileCacheLog,
    published: &mut FxHashSet<u64>,
    cap: usize,
    stats: &mut StcCommitStats,
) -> bool {
    if !log.misses.is_disjoint(published) {
        return false;
    }
    apply_file_log(live, log, published, cap, stats);
    true
}

/// Publishes a validated (or rechecked) file's insertions into the live maps
/// with the same dedup/cap guards the serial intern paths use. Returns whether
/// every insertion was published; a partial publication (cap-blocked or
/// already-present entry) means later files that consumed this file's overlay
/// entries can no longer be validated by digest alone.
pub(crate) fn apply_file_log(
    live: &LiveCacheHandles,
    log: &FileCacheLog,
    published: &mut FxHashSet<u64>,
    cap: usize,
    stats: &mut StcCommitStats,
) -> bool {
    let mut complete = true;

    if !log.generic_inserts.is_empty()
        && let Ok(mut cache) = live.generic.lock()
    {
        for (key, entry, digest) in &log.generic_inserts {
            let bucket = cache.entry(key.clone()).or_default();
            if bucket
                .iter()
                .any(|existing| existing.arguments == entry.arguments)
            {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            if bucket.len() >= cap {
                stats.merge_cap_blocked += 1;
                complete = false;
                continue;
            }
            bucket.push(entry.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }
    if !log.instantiation_inserts.is_empty()
        && let Ok(mut cache) = live.instantiations.lock()
    {
        for (key, entry, digest) in &log.instantiation_inserts {
            let bucket = cache.entry(key.clone()).or_default();
            if bucket
                .iter()
                .any(|existing| existing.arguments == entry.arguments)
            {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            if bucket.len() >= cap {
                stats.merge_cap_blocked += 1;
                complete = false;
                continue;
            }
            bucket.push(entry.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }
    if !log.physical_inserts.is_empty()
        && let Ok(mut cache) = live.physical.lock()
    {
        for (key, resolved, digest) in &log.physical_inserts {
            if cache.contains_key(key) {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            cache.insert(key.clone(), resolved.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }
    if !log.template_inserts.is_empty()
        && let Ok(mut cache) = live.templates.lock()
    {
        for (key, template, digest) in &log.template_inserts {
            if cache.contains_key(key) {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            cache.insert(key.clone(), template.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }
    if !log.method_inserts.is_empty()
        && let Ok(mut cache) = live.methods.lock()
    {
        for (key, function, digest) in &log.method_inserts {
            if cache.contains_key(key) {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            cache.insert(key.clone(), function.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }
    if !log.overload_inserts.is_empty()
        && let Ok(mut cache) = live.overloads.lock()
    {
        for (key, function, digest) in &log.overload_inserts {
            if cache.contains_key(key) {
                stats.merge_skipped_existing += 1;
                complete = false;
                continue;
            }
            cache.insert(key.clone(), function.clone());
            published.insert(*digest);
            stats.published_entries += 1;
        }
    }

    complete
}

// ---------------------------------------------------------------------------
// Publisher-stamped pending reservations
// ---------------------------------------------------------------------------
//
// A replay of position `k` reads the committed store (positions `< frontier`)
// and its own private overlay. When it misses a key that an *earlier* serial
// position will publish but has not committed yet, the resolver recomputes the
// key against an incomplete view — it over-recurses into the declaration body
// and interns spurious structural sub-instantiations, enlarging the digest
// dependency graph and manufacturing false conflicts (see
// `docs/perf/TRPC-ORDERED-DELTA-REPLAY.md` §2).
//
// The reservation table lets a lookup distinguish that case from a genuine
// miss. Before the commit/replay walk, each position reserves — stamped with
// its serial position — the keys its worker log says it will publish. A replay
// at `k` that misses a key in the committed store consults the table: a Pending
// reservation owned by a position `< k` means serial checking would have seen
// that publisher's value, so the replay must *defer* (and be requeued once the
// publisher commits) rather than recompute. Reservations owned by `>= k`
// (future or self) are invisible; a Ready reservation means the value is
// already committed (the store lookup already hit); a Cancelled reservation
// means its owner turned out not to publish the key, so it is not a dependency.
//
// The schedule is derived from worker logs, so it is only a hint: an
// over-reservation makes a later replay defer unnecessarily (it re-checks after
// the publisher commits and finds a hit or a genuine miss — either correct); an
// under-reservation makes a replay compute a key fresh, which the existing
// digest-based commit validation catches and falls back to an inline recheck.
// Imprecision only costs performance, never correctness. Equality is by exact
// key (and exact arguments for the bucketed caches), never by the 64-bit
// conflict digest, so a digest collision cannot merge two distinct keys.

/// Lifecycle of one publisher's claim on a cache key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReservationState {
    /// The owning position intends to publish this key but has not committed.
    Pending,
    /// The owning position committed; the real value is in the live cache.
    Ready,
    /// The owning position's speculative work was discarded without publishing
    /// this key, so it is not a dependency for any later replay.
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct Reservation {
    /// Serial position that owns this reservation.
    publisher: usize,
    /// Attempt/generation that created it, so a stale reservation from a
    /// discarded attempt is distinguishable from a live one.
    attempt: u64,
    state: ReservationState,
    /// Commit version stamped at finalization (0 while Pending/Cancelled).
    version: u64,
}

/// Result of a positional reservation query at replay position `k`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReservationLookup {
    /// No earlier-position publisher reserves this key: a genuine miss (serial
    /// checking at `k` would also miss it), so the caller computes it.
    None,
    /// An earlier position holds a Pending reservation on this key: serial
    /// checking at `k` would observe that publisher's value, so the caller must
    /// defer until the publisher commits rather than recompute.
    Deferred { publisher: usize, attempt: u64 },
}

/// Earliest-Pending-publisher-below-`k` visibility over one key's reservations.
/// `Ready`/`Cancelled` never create a dependency, and a publisher `>= k`
/// (a future position, or the querying position itself) is invisible — so a
/// replay never waits on itself and never observes a future publication.
fn reservation_visibility(
    reservations: impl IntoIterator<Item = Reservation>,
    k: usize,
) -> ReservationLookup {
    let mut best: Option<Reservation> = None;
    for reservation in reservations {
        if reservation.publisher >= k || reservation.state != ReservationState::Pending {
            continue;
        }
        best = match best {
            Some(current) if current.publisher <= reservation.publisher => Some(current),
            _ => Some(reservation),
        };
    }
    match best {
        Some(reservation) => ReservationLookup::Deferred {
            publisher: reservation.publisher,
            attempt: reservation.attempt,
        },
        None => ReservationLookup::None,
    }
}

/// Exact-key publisher reservations kept alongside the six order-visible caches.
/// Bucketed caches (generic, instantiation) match on exact argument vectors,
/// mirroring the caches' own bucket equality; the flat caches key on their
/// `Hash + Eq` instantiation keys. Reservation values are never stored here —
/// the committed value lives in the real cache; this table tracks only the
/// publisher/lifecycle needed to answer the defer-vs-compute question.
#[derive(Default)]
pub(crate) struct ReservationTable {
    generic: FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    instantiations: FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    physical: FxHashMap<InterfaceInstantiationKey, Vec<Reservation>>,
    templates: FxHashMap<StableInterfaceDeclarationId, Vec<Reservation>>,
    methods: FxHashMap<InterfaceMemberInstantiationKey, Vec<Reservation>>,
    overloads: FxHashMap<InterfaceOverloadInstantiationKey, Vec<Reservation>>,
    pending: usize,
    peak_pending: usize,
}

fn reserve_bucketed(
    map: &mut FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    publisher: usize,
    attempt: u64,
    pending: &mut usize,
    peak: &mut usize,
) {
    let bucket = map.entry(key.clone()).or_default();
    if bucket
        .iter()
        .any(|(existing, reservation)| existing == arguments && reservation.publisher == publisher)
    {
        return;
    }
    bucket.push((
        arguments.to_vec(),
        Reservation {
            publisher,
            attempt,
            state: ReservationState::Pending,
            version: 0,
        },
    ));
    *pending += 1;
    *peak = (*peak).max(*pending);
}

fn query_bucketed(
    map: &FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    k: usize,
) -> ReservationLookup {
    match map.get(key) {
        None => ReservationLookup::None,
        Some(bucket) => reservation_visibility(
            bucket
                .iter()
                .filter(|(existing, _)| existing == arguments)
                .map(|(_, reservation)| *reservation),
            k,
        ),
    }
}

fn reserve_flat<K: Clone + Eq + Hash>(
    map: &mut FxHashMap<K, Vec<Reservation>>,
    key: &K,
    publisher: usize,
    attempt: u64,
    pending: &mut usize,
    peak: &mut usize,
) {
    let slot = map.entry(key.clone()).or_default();
    if slot
        .iter()
        .any(|reservation| reservation.publisher == publisher)
    {
        return;
    }
    slot.push(Reservation {
        publisher,
        attempt,
        state: ReservationState::Pending,
        version: 0,
    });
    *pending += 1;
    *peak = (*peak).max(*pending);
}

fn query_flat<K: Eq + Hash>(
    map: &FxHashMap<K, Vec<Reservation>>,
    key: &K,
    k: usize,
) -> ReservationLookup {
    match map.get(key) {
        None => ReservationLookup::None,
        Some(slot) => reservation_visibility(slot.iter().copied(), k),
    }
}

fn transition_bucketed(
    map: &mut FxHashMap<DeclarationResolutionKey, Vec<(Vec<Type>, Reservation)>>,
    publisher: usize,
    to: ReservationState,
    version: u64,
    pending: &mut usize,
) {
    for bucket in map.values_mut() {
        for (_, reservation) in bucket.iter_mut() {
            if reservation.publisher == publisher && reservation.state == ReservationState::Pending
            {
                reservation.state = to;
                reservation.version = version;
                *pending = pending.saturating_sub(1);
            }
        }
    }
}

fn transition_flat<K>(
    map: &mut FxHashMap<K, Vec<Reservation>>,
    publisher: usize,
    to: ReservationState,
    version: u64,
    pending: &mut usize,
) {
    for slot in map.values_mut() {
        for reservation in slot.iter_mut() {
            if reservation.publisher == publisher && reservation.state == ReservationState::Pending
            {
                reservation.state = to;
                reservation.version = version;
                *pending = pending.saturating_sub(1);
            }
        }
    }
}

#[allow(dead_code)]
impl ReservationTable {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reserve_generic(
        &mut self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        publisher: usize,
        attempt: u64,
    ) {
        reserve_bucketed(
            &mut self.generic,
            key,
            arguments,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_generic(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        k: usize,
    ) -> ReservationLookup {
        query_bucketed(&self.generic, key, arguments, k)
    }

    pub(crate) fn reserve_instantiation(
        &mut self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        publisher: usize,
        attempt: u64,
    ) {
        reserve_bucketed(
            &mut self.instantiations,
            key,
            arguments,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_instantiation(
        &self,
        key: &DeclarationResolutionKey,
        arguments: &[Type],
        k: usize,
    ) -> ReservationLookup {
        query_bucketed(&self.instantiations, key, arguments, k)
    }

    pub(crate) fn reserve_physical(
        &mut self,
        key: &InterfaceInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.physical,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_physical(
        &self,
        key: &InterfaceInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.physical, key, k)
    }

    pub(crate) fn reserve_template(
        &mut self,
        key: &StableInterfaceDeclarationId,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.templates,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_template(
        &self,
        key: &StableInterfaceDeclarationId,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.templates, key, k)
    }

    pub(crate) fn reserve_method(
        &mut self,
        key: &InterfaceMemberInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.methods,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_method(
        &self,
        key: &InterfaceMemberInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.methods, key, k)
    }

    pub(crate) fn reserve_overload(
        &mut self,
        key: &InterfaceOverloadInstantiationKey,
        publisher: usize,
        attempt: u64,
    ) {
        reserve_flat(
            &mut self.overloads,
            key,
            publisher,
            attempt,
            &mut self.pending,
            &mut self.peak_pending,
        );
    }

    pub(crate) fn query_overload(
        &self,
        key: &InterfaceOverloadInstantiationKey,
        k: usize,
    ) -> ReservationLookup {
        query_flat(&self.overloads, key, k)
    }

    /// Marks every Pending reservation owned by `publisher` as `Ready` (its
    /// value is now committed). First-writer serial semantics are preserved: an
    /// earlier publisher's reservation is untouched, and a later query resolves
    /// the committed value from the store rather than deferring.
    pub(crate) fn finalize(&mut self, publisher: usize, version: u64) {
        self.transition(publisher, ReservationState::Ready, version);
    }

    /// Marks every Pending reservation owned by `publisher` as `Cancelled`
    /// (its speculative work was discarded without publishing). Dependents that
    /// deferred on it are no longer bound to it — the requeue layer wakes them.
    pub(crate) fn cancel(&mut self, publisher: usize) {
        self.transition(publisher, ReservationState::Cancelled, 0);
    }

    fn transition(&mut self, publisher: usize, to: ReservationState, version: u64) {
        transition_bucketed(&mut self.generic, publisher, to, version, &mut self.pending);
        transition_bucketed(
            &mut self.instantiations,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(
            &mut self.physical,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(
            &mut self.templates,
            publisher,
            to,
            version,
            &mut self.pending,
        );
        transition_flat(&mut self.methods, publisher, to, version, &mut self.pending);
        transition_flat(
            &mut self.overloads,
            publisher,
            to,
            version,
            &mut self.pending,
        );
    }

    /// Live Pending reservations — 0 once the walk has finalized/cancelled every
    /// position (the end-of-run leak assertion).
    pub(crate) fn pending_count(&self) -> usize {
        self.pending
    }

    /// High-water mark of concurrent Pending reservations (peak metadata depth).
    pub(crate) fn peak_pending(&self) -> usize {
        self.peak_pending
    }

    /// Drops all reservation metadata. Pending entries never survive clearing.
    pub(crate) fn clear(&mut self) {
        self.generic.clear();
        self.instantiations.clear();
        self.physical.clear();
        self.templates.clear();
        self.methods.clear();
        self.overloads.clear();
        self.pending = 0;
        self.peak_pending = 0;
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    use crate::context::DeclarationNamespace;

    fn dkey(name: &str) -> DeclarationResolutionKey {
        DeclarationResolutionKey {
            file_name: Arc::from("test.ts"),
            name: Arc::from(name),
            namespace: DeclarationNamespace::Type,
        }
    }

    fn deferred_to(lookup: ReservationLookup) -> Option<usize> {
        match lookup {
            ReservationLookup::Deferred { publisher, .. } => Some(publisher),
            ReservationLookup::None => None,
        }
    }

    // Property 1: an earlier Pending reservation returns Deferred.
    #[test]
    fn earlier_pending_defers() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::Deferred {
                publisher: 2,
                attempt: 10
            }
        );
    }

    // Property 2: an earlier Ready reservation is not a defer (its value is in
    // the committed store, so the store lookup — which runs first — hits).
    #[test]
    fn earlier_ready_is_hit_not_defer() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 10);
        table.finalize(2, 1);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 3: a future (publisher >= k) Pending reservation is invisible.
    #[test]
    fn future_pending_invisible() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 7, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 4: a future Ready reservation is invisible too.
    #[test]
    fn future_ready_invisible() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 7, 10);
        table.finalize(7, 1);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 5: a position's own reservation (publisher == k) does not make it
    // defer on itself.
    #[test]
    fn own_reservation_no_deadlock() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 5, 10);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 5),
            ReservationLookup::None
        );
    }

    // Property 6: with several earlier publishers, the earliest serial position
    // wins.
    #[test]
    fn earliest_serial_position_wins() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 4, 40);
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 3, 30);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
    }

    // Property 7: cancelling a publisher wakes dependents — the next Pending
    // publisher (or a genuine miss) governs after cancellation.
    #[test]
    fn cancel_wakes_dependents() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 3, 30);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        table.cancel(2);
        // Owner 2 no longer binds; the next earlier Pending publisher governs.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(3)
        );
        table.cancel(3);
        // No Pending publisher left below k -> genuine miss.
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            ReservationLookup::None
        );
    }

    // Property 8: finalization (Ready) preserves first-writer semantics — a
    // later query resolves from the store (None here), not a defer, and an
    // unrelated earlier Pending publisher still defers.
    #[test]
    fn ready_preserves_first_writer() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("A"), &[Type::Number], 4, 40);
        table.finalize(2, 1);
        // Earliest is now Ready (committed); publisher 4 still Pending but a
        // query at k=6 sees the committed value via the store, and the table no
        // longer defers to 2. Publisher 4 is < 6 and Pending, so it defers to 4.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(4)
        );
        // At k=3, publisher 4 is a future position -> invisible, no defer.
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 3),
            ReservationLookup::None
        );
    }

    // Property 9: exact argument vectors distinguish bucket entries — a
    // reservation on <Number> does not defer a lookup of <String>.
    #[test]
    fn exact_arguments_distinguish_bucket_entries() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::String], 6),
            ReservationLookup::None
        );
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number, Type::String], 6),
            ReservationLookup::None
        );
    }

    // Property 10: semantic equality is by key/args, never by digest, so keys
    // that a 64-bit digest would collide do not merge. Distinct declaration keys
    // are independent regardless of any hashing.
    #[test]
    fn distinct_keys_do_not_merge() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        // A different declaration key with identical args is a separate entry.
        assert_eq!(
            table.query_generic(&dkey("B"), &[Type::Number], 6),
            ReservationLookup::None
        );
        table.reserve_instantiation(&dkey("A"), &[Type::Number], 3, 30);
        // The generic and instantiation tables are independent caches.
        assert_eq!(
            deferred_to(table.query_generic(&dkey("A"), &[Type::Number], 6)),
            Some(2)
        );
        assert_eq!(
            deferred_to(table.query_instantiation(&dkey("A"), &[Type::Number], 6)),
            Some(3)
        );
    }

    // Property 11: a replay hitting several deferred keys collects distinct
    // publishers (deduplicated).
    #[test]
    fn multiple_dependencies_deduplicate() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_generic(&dkey("B"), &[Type::Number], 2, 20);
        table.reserve_instantiation(&dkey("C"), &[Type::Number], 4, 40);
        let mut publishers = std::collections::BTreeSet::new();
        for lookup in [
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            table.query_generic(&dkey("B"), &[Type::Number], 6),
            table.query_instantiation(&dkey("C"), &[Type::Number], 6),
        ] {
            if let Some(publisher) = deferred_to(lookup) {
                publishers.insert(publisher);
            }
        }
        assert_eq!(publishers.into_iter().collect::<Vec<_>>(), vec![2, 4]);
    }

    // Property 12: clearing removes all reservation metadata, including the
    // Pending count.
    #[test]
    fn clear_removes_all_metadata() {
        let mut table = ReservationTable::new();
        table.reserve_generic(&dkey("A"), &[Type::Number], 2, 20);
        table.reserve_instantiation(&dkey("B"), &[Type::Number], 3, 30);
        assert!(table.pending_count() > 0);
        table.clear();
        assert_eq!(table.pending_count(), 0);
        assert_eq!(table.peak_pending(), 0);
        assert_eq!(
            table.query_generic(&dkey("A"), &[Type::Number], 6),
            ReservationLookup::None
        );
        assert_eq!(
            table.query_instantiation(&dkey("B"), &[Type::Number], 6),
            ReservationLookup::None
        );
    }

    // Flat-cache smoke: the physical instantiation table shares the visibility
    // rule (constructed via a template key, which is a plain Hash+Eq key).
    #[test]
    fn flat_template_cache_defers_and_finalizes() {
        let mut table = ReservationTable::new();
        let key = StableInterfaceDeclarationId {
            canonical_file: Arc::from("lib.d.ts"),
            declaration_start: 100,
            declaration_name: Arc::from("Array"),
            merged_fragments: Arc::from(Vec::new()),
        };
        table.reserve_template(&key, 2, 20);
        assert_eq!(deferred_to(table.query_template(&key, 6)), Some(2));
        table.finalize(2, 1);
        assert_eq!(table.query_template(&key, 6), ReservationLookup::None);
        assert_eq!(table.pending_count(), 0);
    }
}
