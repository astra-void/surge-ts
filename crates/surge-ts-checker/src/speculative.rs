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
}

struct WorkerOverlay {
    generic: FxHashMap<DeclarationResolutionKey, Vec<(usize, GenericInstantiationCacheEntry)>>,
    instantiations: FxHashMap<DeclarationResolutionKey, Vec<(usize, InstantiationCacheEntry)>>,
    physical: FxHashMap<InterfaceInstantiationKey, (usize, Arc<Type>)>,
    templates: FxHashMap<StableInterfaceDeclarationId, (usize, Arc<InterfaceDeclarationTemplate>)>,
    methods: FxHashMap<InterfaceMemberInstantiationKey, (usize, FunctionType)>,
    overloads: FxHashMap<InterfaceOverloadInstantiationKey, (usize, FunctionType)>,
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

/// One worker's speculative view of the six program caches.
pub(crate) struct CheckSession {
    live: LiveCacheHandles,
    base: BaseView,
    state: Mutex<WorkerOverlay>,
}

impl CheckSession {
    pub(crate) fn new(live: LiveCacheHandles, base: Arc<CacheSnapshots>) -> Self {
        Self {
            live,
            base: BaseView::Snapshot(base),
            state: Mutex::new(WorkerOverlay::new()),
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
        }
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
        if let Some(entry) = self.base_instantiation_entry(key, arguments) {
            return Some(entry);
        }
        let mut state = self.state.lock().expect("check session poisoned");
        if let Some((file, entry)) = state.instantiations.get(key).and_then(|bucket| {
            bucket
                .iter()
                .find(|(_, entry)| entry.arguments == arguments)
                .cloned()
        }) {
            state.record_dep(file);
            return Some(entry);
        }
        let digest = digest_bucket(TAG_INSTANTIATION, key, arguments);
        state.current.misses.insert(digest);
        None
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

/// Validates one file's log against everything published so far. On a clean
/// verdict the file's insertions are published into the live maps (in the
/// file's own insertion order) and their digests join `published`.
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
