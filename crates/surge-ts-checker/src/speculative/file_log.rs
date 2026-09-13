use std::sync::{Arc, Mutex};

use surge_ts_types::fx::FxHashSet;
use surge_ts_types::{FunctionType, Type};

use super::{
    GenericMap, InstantiationMap, MethodMap, OverloadMap, PhysicalMap, ReservationTable,
    TemplateMap, display_function_fingerprint, display_type_fingerprint,
};
use crate::context::{
    CheckerContext, DeclarationResolutionKey, GenericInstantiationCacheEntry,
    InstantiationCacheEntry, InterfaceDeclarationTemplate, InterfaceInstantiationKey,
    InterfaceMemberInstantiationKey, InterfaceOverloadInstantiationKey,
    StableInterfaceDeclarationId,
};

/// The live shared cache maps, held so the session can (a) recognize contexts
/// whose handles are the live ones by pointer identity and (b) publish into
/// them at commit time.
#[derive(Clone)]
pub(crate) struct LiveCacheHandles {
    pub(super) generic: Arc<Mutex<GenericMap>>,
    pub(super) instantiations: Arc<Mutex<InstantiationMap>>,
    pub(super) physical: Arc<Mutex<PhysicalMap>>,
    pub(super) templates: Arc<Mutex<TemplateMap>>,
    pub(super) methods: Arc<Mutex<MethodMap>>,
    pub(super) overloads: Arc<Mutex<OverloadMap>>,
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
    pub(super) generic: GenericMap,
    pub(super) instantiations: InstantiationMap,
    pub(super) physical: PhysicalMap,
    pub(super) templates: TemplateMap,
    pub(super) methods: MethodMap,
    pub(super) overloads: OverloadMap,
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
    pub(super) misses: FxHashSet<u64>,
    /// Files (same worker, earlier in file order) whose overlay insertions this
    /// file's lookups hit. If any of them fails validation, this file's hits may
    /// not match serial and it must be rechecked too.
    pub(super) overlay_deps: FxHashSet<usize>,
    pub(super) generic_inserts: Vec<(
        DeclarationResolutionKey,
        GenericInstantiationCacheEntry,
        u64,
    )>,
    pub(super) instantiation_inserts: Vec<(DeclarationResolutionKey, InstantiationCacheEntry, u64)>,
    pub(super) physical_inserts: Vec<(InterfaceInstantiationKey, Arc<Type>, u64)>,
    pub(super) template_inserts: Vec<(
        StableInterfaceDeclarationId,
        Arc<InterfaceDeclarationTemplate>,
        u64,
    )>,
    pub(super) method_inserts: Vec<(InterfaceMemberInstantiationKey, FunctionType, u64)>,
    pub(super) overload_inserts: Vec<(InterfaceOverloadInstantiationKey, FunctionType, u64)>,
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
