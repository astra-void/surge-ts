//! The engine lock and the process-global "current program" state it guards.
//!
//! The checker keeps parts of the program it is checking in process globals
//! (module-resolution policy, the published module scopes and ambient globals,
//! the program-lifetime instantiation memo, the check-phase marker) and in
//! thread-local caches. One check at a time, one program alive at a time, is
//! the only model that state supports, so every check and every query runs
//! under [`engine_lock`]; a retained program re-installs its own state each
//! time it is entered and takes the memo back out when it leaves.

use std::cell::Cell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use surge_ts_types::Type;
use surge_ts_types::fx::FxHashMap;

use crate::context::DeclarationResolutionKey;
use crate::symbols::{SymbolTable, TypeDeclarationScope};

static ENGINE_LOCK: Mutex<()> = Mutex::new(());
static NEXT_PROGRAM_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// The program whose state this thread's caches were last filled for.
    static THREAD_PROGRAM: Cell<u64> = const { Cell::new(0) };
}

/// Serializes every check and query in the process. A panic inside a query
/// poisons nothing the next one relies on: each entry re-installs its state.
pub(crate) fn engine_lock() -> MutexGuard<'static, ()> {
    ENGINE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn next_program_id() -> u64 {
    NEXT_PROGRAM_ID.fetch_add(1, Ordering::Relaxed)
}

/// Marks this thread's caches as belonging to `program` (0 for a one-shot
/// run), clearing them first when they were filled for another one.
pub(crate) fn claim_thread_caches(program: u64) {
    if THREAD_PROGRAM.with(Cell::get) == program {
        return;
    }
    crate::paths::clear_canonicalize_cache();
    crate::modules::clear_relative_module_cache();
    crate::modules::clear_star_export_unresolved_cache();
    crate::modules::clear_namespace_alias_table_cache();
    crate::infer::types::clear_deferred_merges();
    THREAD_PROGRAM.with(|current| current.set(program));
}

pub(crate) type ModuleScopes = Arc<FxHashMap<Arc<str>, Arc<TypeDeclarationScope>>>;

/// What a retained program installs into the process globals on entry.
pub(crate) struct ProgramGlobals {
    pub(crate) node_esm_files: Option<Arc<HashSet<String>>>,
    pub(crate) allow_arbitrary_extensions: bool,
    pub(crate) module_scopes: Option<ModuleScopes>,
    pub(crate) ambient_globals: Arc<SymbolTable>,
    /// The program-lifetime instantiation memo, parked here while the program
    /// is not entered.
    pub(crate) memo: Mutex<FxHashMap<DeclarationResolutionKey, Type>>,
}

/// Installs `globals` for the duration of `f`, then parks the memo again.
/// The caller holds the engine lock.
pub(crate) fn with_program_globals<R>(program: u64, globals: &ProgramGlobals, f: impl FnOnce() -> R) -> R {
    struct Exit<'a> {
        globals: &'a ProgramGlobals,
    }
    impl Drop for Exit<'_> {
        fn drop(&mut self) {
            crate::program::set_check_phase(false);
            if let Ok(mut parked) = self.globals.memo.lock() {
                crate::infer::types::cache::swap_program_module_instantiation_memo(&mut parked);
            }
        }
    }
    claim_thread_caches(program);
    crate::modules::set_node_esm_files(globals.node_esm_files.clone());
    crate::modules::set_allow_arbitrary_extensions(globals.allow_arbitrary_extensions);
    match &globals.module_scopes {
        Some(scopes) => crate::program::publish_program_module_scopes(scopes),
        None => crate::program::clear_program_module_scopes(),
    }
    crate::program::publish_program_ambient_globals(globals.ambient_globals.clone());
    if let Ok(mut parked) = globals.memo.lock() {
        crate::infer::types::cache::swap_program_module_instantiation_memo(&mut parked);
    }
    let _exit = Exit { globals };
    crate::program::set_check_phase(true);
    f()
}
