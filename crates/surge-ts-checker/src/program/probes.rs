use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.

use crate::modules::ModuleImportBindings;
use crate::symbols::{
    SymbolTable, TypeDeclarationScope, TypeDeclarationTable,
};
use super::{ModuleAnalysis, ParsedProgramFile};

pub(crate) fn eq_probe_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_EQ_STATS").is_some())
}

/// Per-round, per-file counter samples taken around one module's analysis.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EqProbeVisit {
    pub(crate) elapsed: std::time::Duration,
    pub(crate) signature_scope_consults: u64,
    pub(crate) degraded_resolutions: u64,
    pub(crate) augmentation_insertions_after: u64,
}

pub(super) static EQ_PROBE_VISITS: std::sync::OnceLock<Mutex<HashMap<usize, Vec<EqProbeVisit>>>> =
    std::sync::OnceLock::new();

pub(crate) fn record_eq_probe_visit(file_index: usize, visit: EqProbeVisit) {
    let store = EQ_PROBE_VISITS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut visits) = store.lock() {
        visits.entry(file_index).or_default().push(visit);
    }
}

pub(super) static SCOPE_FALLBACK_CONSULTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_scope_fallback_consult() {
    SCOPE_FALLBACK_CONSULTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn scope_fallback_consult_count() -> u64 {
    SCOPE_FALLBACK_CONSULTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Degraded (`had_error`, uninternable) named-type resolutions. A file whose
/// preliminary analysis observed one may resolve differently in the final round
/// (a clean interned entry can exist by then), so the equality predictor must
/// exclude it.
pub(super) static DEGRADED_RESOLUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_degraded_resolution() {
    DEGRADED_RESOLUTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn degraded_resolution_count() -> u64 {
    DEGRADED_RESOLUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// First-wins `declare global` value insertions (`lower_global_augmentation_values`).
/// They mutate `ctx.ambient_global_symbols` *during* the analysis loop, so a file
/// analyzed before the inserting file saw fewer globals in the preliminary round
/// than it will in the final round.
pub(super) static AUGMENTATION_VALUE_INSERTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_augmentation_value_insertion() {
    AUGMENTATION_VALUE_INSERTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn augmentation_value_insertion_count() -> u64 {
    AUGMENTATION_VALUE_INSERTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Opt-in per-module analysis-time dump (`SURGE_MODULE_TIME_DUMP=<path>`): one
/// `round\tmicros\tfile_name` line per analyzed module, so the real per-module
/// analysis cost distribution can be joined with the import-edge dump for
/// SCC critical-path weighting. Off by default; zero-cost when unset.
pub(super) fn module_time_sink() -> Option<&'static Mutex<std::fs::File>> {
    static SINK: std::sync::OnceLock<Option<Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::env::var_os("SURGE_MODULE_TIME_DUMP")?;
        std::fs::File::create(path).ok().map(Mutex::new)
    })
    .as_ref()
}

pub(crate) fn module_time_dump_enabled() -> bool {
    module_time_sink().is_some()
}

pub(crate) fn record_module_time(round: u64, file_name: &str, micros: u128) {
    use std::io::Write;
    if let Some(sink) = module_time_sink()
        && let Ok(mut file) = sink.lock()
    {
        let _ = writeln!(file, "{round}\t{micros}\t{file_name}");
    }
}

pub(super) fn eq_probe_verbose() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_EQ_STATS_VERBOSE").is_some())
}

pub(super) fn eq_probe_symbol_tables_equal(a: &SymbolTable, b: &SymbolTable) -> bool {
    let count_a = a.iter_shared().count();
    if count_a != b.iter_shared().count() {
        return false;
    }
    a.iter_shared().all(|(name, symbol_a)| {
        b.get_shared(name)
            .is_some_and(|symbol_b| symbol_a.kind == symbol_b.kind && symbol_a.ty == symbol_b.ty)
    })
}

pub(super) fn eq_probe_explain_symbol_tables(label: &str, a: &SymbolTable, b: &SymbolTable) {
    let count_a = a.iter_shared().count();
    let count_b = b.iter_shared().count();
    if count_a != count_b {
        eprintln!("[eq-stats]   {label}: entry count {count_a} vs {count_b}");
        let names_a: HashSet<&str> = a.iter_shared().map(|(n, _)| n.as_ref()).collect();
        let names_b: HashSet<&str> = b.iter_shared().map(|(n, _)| n.as_ref()).collect();
        for only_a in names_a.difference(&names_b) {
            eprintln!("[eq-stats]     only-prelim: {only_a}");
        }
        for only_b in names_b.difference(&names_a) {
            eprintln!("[eq-stats]     only-final: {only_b}");
        }
        return;
    }
    for (name, symbol_a) in a.iter_shared() {
        match b.get_shared(name) {
            None => eprintln!("[eq-stats]   {label}: '{name}' missing in final"),
            Some(symbol_b) => {
                if symbol_a.kind != symbol_b.kind {
                    eprintln!(
                        "[eq-stats]   {label}: '{name}' kind {:?} vs {:?}",
                        symbol_a.kind, symbol_b.kind
                    );
                } else if symbol_a.ty != symbol_b.ty {
                    eprintln!(
                        "[eq-stats]   {label}: '{name}' type '{}' vs '{}'",
                        symbol_a.ty.name(),
                        symbol_b.ty.name()
                    );
                }
            }
        }
    }
}

pub(super) fn eq_probe_explain_analyses(file_name: &str, a: &ModuleAnalysis, b: &ModuleAnalysis) {
    eprintln!("[eq-stats]  explain {file_name}:");
    eq_probe_explain_symbol_tables("local_symbols", &a.local_symbols, &b.local_symbols);
    eq_probe_explain_symbol_tables(
        "export_symbols",
        &a.local_export_table.symbols,
        &b.local_export_table.symbols,
    );
    let table_a = &a.local_export_table;
    let table_b = &b.local_export_table;
    match (&table_a.default_symbol, &table_b.default_symbol) {
        (Some(sa), Some(sb)) if sa.ty != sb.ty => eprintln!(
            "[eq-stats]   default_symbol: '{}' vs '{}'",
            sa.ty.name(),
            sb.ty.name()
        ),
        (Some(_), None) | (None, Some(_)) => eprintln!("[eq-stats]   default_symbol presence"),
        _ => {}
    }
    match (
        &table_a.export_assignment_symbol,
        &table_b.export_assignment_symbol,
    ) {
        (Some(sa), Some(sb)) if sa.ty != sb.ty => eprintln!(
            "[eq-stats]   export_assignment: '{}' vs '{}'",
            sa.ty.name(),
            sb.ty.name()
        ),
        (Some(_), None) | (None, Some(_)) => eprintln!("[eq-stats]   export_assignment presence"),
        _ => {}
    }
    if table_a.type_declarations.len() != table_b.type_declarations.len() {
        eprintln!(
            "[eq-stats]   export type_declarations: {} vs {}",
            table_a.type_declarations.len(),
            table_b.type_declarations.len()
        );
    }
}

pub(super) fn eq_probe_analyses_equal(a: &ModuleAnalysis, b: &ModuleAnalysis) -> bool {
    let table_a = &a.local_export_table;
    let table_b = &b.local_export_table;
    eq_probe_symbol_tables_equal(&a.local_symbols, &b.local_symbols)
        && eq_probe_symbol_tables_equal(&table_a.symbols, &table_b.symbols)
        && match (&table_a.default_symbol, &table_b.default_symbol) {
            (None, None) => true,
            (Some(sa), Some(sb)) => sa.ty == sb.ty,
            _ => false,
        }
        && match (
            &table_a.export_assignment_symbol,
            &table_b.export_assignment_symbol,
        ) {
            (None, None) => true,
            (Some(sa), Some(sb)) => sa.ty == sb.ty,
            _ => false,
        }
        && table_a.type_declarations.len() == table_b.type_declarations.len()
}

pub(super) fn eq_probe_scopes_equal(
    a: &Option<Arc<TypeDeclarationScope>>,
    b: &Option<Arc<TypeDeclarationScope>>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            Arc::ptr_eq(a, b) || {
                let (la, lb) = (a.layers(), b.layers());
                la.len() == lb.len() && la.iter().zip(lb).all(|(x, y)| Arc::ptr_eq(x, y))
            }
        }
        _ => false,
    }
}

pub(super) fn eq_probe_declarations_equal(
    a: &crate::symbols::TypeDeclarationInfo,
    b: &crate::symbols::TypeDeclarationInfo,
) -> bool {
    use crate::symbols::TypeDeclarationInfo;
    match (a, b) {
        (TypeDeclarationInfo::Alias(a), TypeDeclarationInfo::Alias(b)) => {
            a.name == b.name
                && Arc::ptr_eq(&a.body, &b.body)
                && eq_probe_scopes_equal(&a.resolution_scope, &b.resolution_scope)
        }
        (TypeDeclarationInfo::Interface(a), TypeDeclarationInfo::Interface(b)) => {
            a.name == b.name
                && Arc::ptr_eq(&a.body, &b.body)
                && eq_probe_scopes_equal(&a.resolution_scope, &b.resolution_scope)
        }
        _ => false,
    }
}

pub(super) fn eq_probe_declaration_tables_equal(a: &TypeDeclarationTable, b: &TypeDeclarationTable) -> bool {
    a.len() == b.len()
        && a.iter().all(|(name, da)| {
            b.get(name.as_ref())
                .is_some_and(|db| eq_probe_declarations_equal(da, db))
        })
}

pub(super) fn eq_probe_layer_equal(a: &Arc<TypeDeclarationTable>, b: &Arc<TypeDeclarationTable>) -> bool {
    Arc::ptr_eq(a, b) || eq_probe_declaration_tables_equal(a, b)
}

pub(super) fn eq_probe_bindings_equal(a: &ModuleImportBindings, b: &ModuleImportBindings) -> bool {
    eq_probe_symbol_tables_equal(&a.symbols, &b.symbols)
        && eq_probe_layer_equal(&a.type_declarations, &b.type_declarations)
        && a.namespace_alias_layers.len() == b.namespace_alias_layers.len()
        && a.namespace_alias_layers
            .iter()
            .zip(&b.namespace_alias_layers)
            .all(|(x, y)| eq_probe_layer_equal(x, y))
}

pub(super) fn report_eq_probe(
    parsed_files: &[ParsedProgramFile],
    preliminary: &[Option<ModuleAnalysis>],
    final_analyses: &[Option<ModuleAnalysis>],
    preliminary_bindings: &[Option<ModuleImportBindings>],
    final_bindings: &[Option<ModuleImportBindings>],
    augmentation_insertions_before_final: u64,
) {
    if !eq_probe_enabled() {
        return;
    }
    let visits_by_file = EQ_PROBE_VISITS
        .get()
        .and_then(|store| store.lock().ok().map(|d| d.clone()))
        .unwrap_or_default();
    let mut analyzed = 0usize;
    let mut equal = 0usize;
    let mut predicted = 0usize;
    let mut unsound = 0usize;
    let mut excluded_consults = 0usize;
    let mut excluded_degraded = 0usize;
    let mut excluded_augmentation = 0usize;
    let mut excluded_bindings = 0usize;
    let mut prelim_total = std::time::Duration::ZERO;
    let mut prelim_equal = std::time::Duration::ZERO;
    let mut final_total = std::time::Duration::ZERO;
    let mut final_equal = std::time::Duration::ZERO;
    let mut final_predicted = std::time::Duration::ZERO;
    for (index, (p, f)) in preliminary.iter().zip(final_analyses.iter()).enumerate() {
        let (Some(p), Some(f)) = (p, f) else { continue };
        analyzed += 1;
        let visits = visits_by_file.get(&index);
        let prelim_visit = visits.and_then(|v| v.first().copied()).unwrap_or_default();
        let final_visit = visits.and_then(|v| v.get(1).copied()).unwrap_or_default();
        let prelim_time = prelim_visit.elapsed;
        let final_time = final_visit.elapsed;
        prelim_total += prelim_time;
        final_total += final_time;
        let bindings_equal = match (&preliminary_bindings[index], &final_bindings[index]) {
            (Some(a), Some(b)) => eq_probe_bindings_equal(a, b),
            (None, None) => true,
            _ => false,
        };
        let output_equal = eq_probe_analyses_equal(p, f);
        if !bindings_equal {
            excluded_bindings += 1;
        } else if prelim_visit.signature_scope_consults != 0 {
            excluded_consults += 1;
        } else if prelim_visit.degraded_resolutions != 0 {
            excluded_degraded += 1;
        } else if prelim_visit.augmentation_insertions_after != augmentation_insertions_before_final
        {
            excluded_augmentation += 1;
        }
        let is_predicted = bindings_equal
            && prelim_visit.signature_scope_consults == 0
            && prelim_visit.degraded_resolutions == 0
            && prelim_visit.augmentation_insertions_after == augmentation_insertions_before_final;
        if output_equal {
            equal += 1;
            prelim_equal += prelim_time;
            final_equal += final_time;
        }
        if is_predicted {
            predicted += 1;
            final_predicted += final_time;
            if !output_equal {
                unsound += 1;
                eprintln!(
                    "[eq-stats] UNSOUND predicted-equal but output differs: {}",
                    parsed_files[index].file_name
                );
                if eq_probe_verbose() {
                    eq_probe_explain_analyses(&parsed_files[index].file_name, p, f);
                }
            }
        }
    }
    eprintln!(
        "[eq-stats] excluded: bindings={excluded_bindings} consults={excluded_consults} \
         degraded={excluded_degraded} augmentation={excluded_augmentation}"
    );
    eprintln!(
        "[eq-stats] analyzed={analyzed} output_equal={equal} ({:.1}%) \
         prelim_time_equal={:.2}s/{:.2}s ({:.1}%) final_time_equal={:.2}s/{:.2}s ({:.1}%)",
        100.0 * equal as f64 / analyzed.max(1) as f64,
        prelim_equal.as_secs_f64(),
        prelim_total.as_secs_f64(),
        100.0 * prelim_equal.as_secs_f64() / prelim_total.as_secs_f64().max(f64::EPSILON),
        final_equal.as_secs_f64(),
        final_total.as_secs_f64(),
        100.0 * final_equal.as_secs_f64() / final_total.as_secs_f64().max(f64::EPSILON),
    );
    eprintln!(
        "[eq-stats] predicted_skip={predicted} ({:.1}%) unsound={unsound} \
         final_time_predicted={:.2}s/{:.2}s ({:.1}%)",
        100.0 * predicted as f64 / analyzed.max(1) as f64,
        final_predicted.as_secs_f64(),
        final_total.as_secs_f64(),
        100.0 * final_predicted.as_secs_f64() / final_total.as_secs_f64().max(f64::EPSILON),
    );
}
