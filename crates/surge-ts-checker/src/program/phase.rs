use std::sync::Arc;

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.

/// Program-global check-phase marker. A `CheckerContext` field cannot carry
/// this: most check-phase resolution runs inside environment-RECOVERED
/// contexts (`from_declaration_environment`), which would never see a field
/// set on the driving context. Set when the per-file check dispatch starts,
/// cleared in the end-of-run teardown alongside `clear_program_type_caches`.
pub(super) static IN_CHECK_PHASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_check_phase(on: bool) {
    IN_CHECK_PHASE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn in_check_phase() -> bool {
    IN_CHECK_PHASE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Program-global copy of the authoritative per-file scope map, published when
/// the driving context installs it. Same reason as [`IN_CHECK_PHASE`]: a lazy
/// value/signature annotation resolves inside an environment-RECOVERED context,
/// and an environment captured before the map existed (module analysis takes it
/// away — see `binding.rs`) would otherwise resolve the declaring file's own
/// imports against an empty map. A dependency `.d.ts` that names an imported
/// type (lucide's `import { SVGProps } from 'react'`) then missed at check time.
/// Keyed by file name and rebuilt per program, so consulting it is the same
/// authoritative fallback `module_scope_for_file` already provides. Released in
/// the end-of-run teardown.
pub(super) static PROGRAM_MODULE_SCOPES: std::sync::Mutex<
    Option<Arc<surge_ts_types::fx::FxHashMap<Arc<str>, Arc<crate::symbols::TypeDeclarationScope>>>>,
> = std::sync::Mutex::new(None);

pub(crate) fn publish_program_module_scopes(
    scopes: &Arc<surge_ts_types::fx::FxHashMap<Arc<str>, Arc<crate::symbols::TypeDeclarationScope>>>,
) {
    if let Ok(mut slot) = PROGRAM_MODULE_SCOPES.lock() {
        *slot = Some(scopes.clone());
    }
}

pub(crate) fn program_module_scope_for_file(
    file_name: &str,
) -> Option<Arc<crate::symbols::TypeDeclarationScope>> {
    let slot = PROGRAM_MODULE_SCOPES.lock().ok()?;
    slot.as_ref()?.get(file_name).cloned()
}

pub(crate) fn clear_program_module_scopes() {
    if let Ok(mut slot) = PROGRAM_MODULE_SCOPES.lock() {
        *slot = None;
    }
}
