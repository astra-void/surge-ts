use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use surge_ts_types::{snapshot_function_type_counters, snapshot_union_type_counters};

use super::{current_footprint_bytes, peak_footprint_bytes, snapshot_program_counters};
use crate::context::DeclarationResolutionKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum DtsExpansionReason {
    SignatureParameter,
    SignatureReturn,
    SignatureThisParameter,
    SignatureTypePredicate,
    GenericConstraint,
    GenericDefault,
    CallSignature,
    ConstructSignature,
    InterfaceMethod,
    ClassMethod,
    ClassConstructor,
    FunctionTypeAnnotation,
    ModuleExportCollection,
    OverloadResolution,
    CallResolution,
    ConstructResolution,
    Assignability,
    ContextualTyping,
    GenericInference,
    PropertyLookup,
    IndexedAccess,
    ConditionalType,
    MappedType,
    IntersectionMerge,
    UnionNormalization,
    ApparentType,
    DiagnosticDisplay,
    ModuleDedup,
    Other,
}

impl DtsExpansionReason {
    fn label(self) -> &'static str {
        match self {
            Self::SignatureParameter => "signature_parameter",
            Self::SignatureReturn => "signature_return",
            Self::SignatureThisParameter => "signature_this_parameter",
            Self::SignatureTypePredicate => "signature_type_predicate",
            Self::GenericConstraint => "generic_constraint",
            Self::GenericDefault => "generic_default",
            Self::CallSignature => "call_signature",
            Self::ConstructSignature => "construct_signature",
            Self::InterfaceMethod => "interface_method",
            Self::ClassMethod => "class_method",
            Self::ClassConstructor => "class_constructor",
            Self::FunctionTypeAnnotation => "function_type_annotation",
            Self::ModuleExportCollection => "module_export_collection",
            Self::OverloadResolution => "overload_resolution",
            Self::CallResolution => "call_resolution",
            Self::ConstructResolution => "construct_resolution",
            Self::Assignability => "assignability",
            Self::ContextualTyping => "contextual_typing",
            Self::GenericInference => "generic_inference",
            Self::PropertyLookup => "property_lookup",
            Self::IndexedAccess => "indexed_access",
            Self::ConditionalType => "conditional_type",
            Self::MappedType => "mapped_type",
            Self::IntersectionMerge => "intersection_merge",
            Self::UnionNormalization => "union_normalization",
            Self::ApparentType => "apparent_type",
            Self::DiagnosticDisplay => "diagnostic_display",
            Self::ModuleDedup => "module_dedup",
            Self::Other => "other",
        }
    }
}

thread_local! {
    static CURRENT_REASON: Cell<DtsExpansionReason> = const { Cell::new(DtsExpansionReason::Other) };
}

pub(crate) fn with_dts_expansion_reason<T>(reason: DtsExpansionReason, f: impl FnOnce() -> T) -> T {
    CURRENT_REASON.with(|current| {
        let previous = current.replace(reason);
        let result = f();
        current.set(previous);
        result
    })
}

pub(crate) fn current_dts_expansion_reason() -> DtsExpansionReason {
    CURRENT_REASON.get()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypeCreationSnapshot {
    object_types: u64,
    function_types: u64,
    union_types: u64,
}

pub(crate) fn type_creation_snapshot() -> TypeCreationSnapshot {
    let program = snapshot_program_counters();
    let functions = snapshot_function_type_counters();
    let unions = snapshot_union_type_counters();
    TypeCreationSnapshot {
        object_types: program.arena_object_type_payload_alloc_count,
        function_types: functions.function_type_payload_alloc_count,
        union_types: unions.union_type_payload_alloc_count,
    }
}

#[derive(Default)]
struct ExpansionTotals {
    structural_expansions: u64,
    object_types: u64,
    function_types: u64,
    union_types: u64,
}

#[derive(Default)]
struct ExpansionTrace {
    high_water_bytes: u64,
    created_by_declaration: HashMap<String, u64>,
    peels_by_declaration: HashMap<String, u64>,
    peels_by_reason_and_declaration: HashMap<(DtsExpansionReason, String), u64>,
    generic_instantiations_by_declaration: HashMap<String, u64>,
    expansions_by_declaration: HashMap<String, ExpansionTotals>,
    expansions_by_file: HashMap<String, ExpansionTotals>,
    peel_reasons: HashMap<DtsExpansionReason, u64>,
    retained_nodes_by_module: HashMap<String, u64>,
    degraded_signature_attempts_by_declaration: HashMap<String, u64>,
}

static TRACE: OnceLock<Mutex<ExpansionTrace>> = OnceLock::new();

pub(crate) fn dts_expansion_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_TRACE_DTS_EXPANSION").is_some())
}

pub(crate) fn reset_dts_expansion_trace() {
    if let Some(trace) = TRACE.get()
        && let Ok(mut trace) = trace.lock()
    {
        *trace = ExpansionTrace::default();
    }
}

pub(crate) fn record_lazy_reference_created(key: &DeclarationResolutionKey) {
    crate::program::record_program_counter(|c| c.lazy_reference_create_count += 1);
    if !dts_expansion_trace_enabled() {
        return;
    }
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    *trace
        .created_by_declaration
        .entry(declaration_label(key))
        .or_default() += 1;
}

pub(crate) fn record_generic_instantiation(key: &DeclarationResolutionKey) {
    crate::program::record_program_counter(|c| c.generic_instantiation_count += 1);
    if !dts_expansion_trace_enabled() {
        return;
    }
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    *trace
        .generic_instantiations_by_declaration
        .entry(declaration_label(key))
        .or_default() += 1;
}

pub(crate) fn record_lazy_reference_peel_start(key: &DeclarationResolutionKey) {
    let reason = current_dts_expansion_reason();
    crate::program::record_program_counter(|c| {
        c.lazy_reference_peel_count += 1;
        c.lazy_reference_peel_reason_counts[reason as usize] += 1;
    });
    if !dts_expansion_trace_enabled() {
        return;
    }
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    *trace
        .peels_by_declaration
        .entry(declaration_label(key))
        .or_default() += 1;
    *trace
        .peels_by_reason_and_declaration
        .entry((reason, declaration_label(key)))
        .or_default() += 1;
    *trace.peel_reasons.entry(reason).or_default() += 1;
}

pub(crate) fn record_degraded_signature_expansion(key: &DeclarationResolutionKey) {
    if !super::counters_enabled() {
        return;
    }
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    let attempts = trace
        .degraded_signature_attempts_by_declaration
        .entry(declaration_label(key))
        .or_default();
    *attempts += 1;
    let attempts = *attempts;
    crate::program::record_program_counter(|c| {
        if attempts == 1 {
            c.unique_degraded_signature_expansion_count += 1;
        } else {
            c.repeated_degraded_signature_expansion_count += 1;
            c.max_degraded_signature_expansion_repeats =
                c.max_degraded_signature_expansion_repeats.max(attempts);
        }
    });
}

pub(crate) fn record_lazy_reference_expansion(
    key: &DeclarationResolutionKey,
    module: &str,
    display: &str,
    resolution_depth: usize,
    before: TypeCreationSnapshot,
) {
    crate::program::record_program_counter(|c| c.lazy_reference_clean_expansion_count += 1);
    let after = type_creation_snapshot();
    if !dts_expansion_trace_enabled() {
        return;
    }

    let objects = after.object_types.saturating_sub(before.object_types);
    let functions = after.function_types.saturating_sub(before.function_types);
    let unions = after.union_types.saturating_sub(before.union_types);
    let current_footprint = current_footprint_bytes();
    let peak_footprint = peak_footprint_bytes();
    let reason = current_dts_expansion_reason();
    let label = declaration_label(key);

    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    add_expansion(
        trace
            .expansions_by_declaration
            .entry(label.clone())
            .or_default(),
        objects,
        functions,
        unions,
    );
    add_expansion(
        trace
            .expansions_by_file
            .entry(key.file_name.clone())
            .or_default(),
        objects,
        functions,
        unions,
    );

    let high_water = peak_footprint.or(current_footprint).unwrap_or(0);
    if high_water > trace.high_water_bytes {
        trace.high_water_bytes = high_water;
        eprintln!(
            "{{\"dtsExpansionHighWater\":true,\"footprintBytes\":{},\"peakFootprintBytes\":{},\
             \"file\":{:?},\"module\":{:?},\"exportName\":{:?},\"operation\":{:?},\
             \"declarationKind\":\"named_type\",\"referenceId\":{:?},\
             \"resolutionDepth\":{},\"typesCreated\":{},\"objectTypesCreated\":{},\
             \"functionTypesCreated\":{},\"unionTypesCreated\":{}}}",
            current_footprint.map_or_else(|| "null".to_string(), |v| v.to_string()),
            peak_footprint.map_or_else(|| "null".to_string(), |v| v.to_string()),
            key.file_name,
            module,
            display,
            reason.label(),
            label,
            resolution_depth,
            objects + functions + unions,
            objects,
            functions,
            unions,
        );
    }
}

pub(crate) fn record_lazy_reference_expansion_start(
    key: &DeclarationResolutionKey,
    module: &str,
    display: &str,
    resolution_depth: usize,
) {
    if !dts_expansion_trace_enabled() {
        return;
    }
    let current = current_footprint_bytes();
    let peak = peak_footprint_bytes();
    let high_water = peak.or(current).unwrap_or(0);
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    if high_water <= trace.high_water_bytes {
        return;
    }
    trace.high_water_bytes = high_water;
    eprintln!(
        "{{\"dtsExpansionHighWater\":true,\"footprintBytes\":{},\"peakFootprintBytes\":{},\
         \"file\":{:?},\"module\":{:?},\"exportName\":{:?},\
         \"operation\":{:?},\"declarationKind\":\"named_type\",\
         \"referenceId\":{:?},\"resolutionDepth\":{},\"typesCreated\":0,\
         \"objectTypesCreated\":0,\"functionTypesCreated\":0,\
         \"unionTypesCreated\":0}}",
        current.map_or_else(|| "null".to_string(), |v| v.to_string()),
        peak.map_or_else(|| "null".to_string(), |v| v.to_string()),
        key.file_name,
        module,
        display,
        current_dts_expansion_reason().label(),
        declaration_label(key),
        resolution_depth,
    );
}

pub(crate) fn record_retained_export_nodes(module: &str, nodes: u64) {
    if !dts_expansion_trace_enabled() {
        return;
    }
    let mut trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    trace
        .retained_nodes_by_module
        .insert(module.to_string(), nodes);
}

pub(crate) fn render_dts_expansion_summary() {
    if !dts_expansion_trace_enabled() {
        return;
    }
    let trace = TRACE
        .get_or_init(|| Mutex::new(ExpansionTrace::default()))
        .lock()
        .expect("d.ts expansion trace poisoned");
    eprintln!("d.ts expansion summary:");
    render_expansion_top(
        "top files by object types created",
        &trace.expansions_by_file,
        |v| v.object_types,
    );
    render_expansion_top(
        "top declarations by structural expansion count",
        &trace.expansions_by_declaration,
        |v| v.structural_expansions,
    );
    render_expansion_top(
        "top declarations by function types created",
        &trace.expansions_by_declaration,
        |v| v.function_types,
    );
    render_expansion_top_filtered(
        "top physical default-lib declarations by function types created",
        &trace.expansions_by_declaration,
        |name| {
            declaration_file(name)
                .is_some_and(crate::default_lib::is_physical_default_lib_file_name)
        },
        |v| v.function_types,
    );
    render_expansion_top_filtered(
        "top external dependency declarations by function types created",
        &trace.expansions_by_declaration,
        |name| {
            declaration_file(name).is_some_and(|file| {
                file.contains("node_modules")
                    && !crate::default_lib::is_physical_default_lib_file_name(file)
            })
        },
        |v| v.function_types,
    );
    render_count_top(
        "top lazy references by peel count",
        &trace.peels_by_declaration,
    );
    render_reason_count_top(
        "top lazy references peeled during module export collection",
        &trace.peels_by_reason_and_declaration,
        DtsExpansionReason::ModuleExportCollection,
    );
    render_count_top(
        "top generic aliases by instantiation count",
        &trace.generic_instantiations_by_declaration,
    );
    render_count_top(
        "top modules by retained export-table type nodes",
        &trace.retained_nodes_by_module,
    );
    render_count_top(
        "top degraded signature annotations by expansion attempts",
        &trace.degraded_signature_attempts_by_declaration,
    );
    let mut reasons: Vec<_> = trace.peel_reasons.iter().collect();
    reasons.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    eprintln!("  lazy-reference peel reasons:");
    for (reason, count) in reasons {
        eprintln!("    {}: {count}", reason.label());
    }
}

fn declaration_label(key: &DeclarationResolutionKey) -> String {
    format!("{}::{}", key.file_name, key.name)
}

fn declaration_file(label: &str) -> Option<&str> {
    label.rsplit_once("::").map(|(file, _)| file)
}

fn add_expansion(totals: &mut ExpansionTotals, objects: u64, functions: u64, unions: u64) {
    totals.structural_expansions += 1;
    totals.object_types += objects;
    totals.function_types += functions;
    totals.union_types += unions;
}

fn render_expansion_top(
    title: &str,
    values: &HashMap<String, ExpansionTotals>,
    value: impl Fn(&ExpansionTotals) -> u64,
) {
    let mut rows: Vec<_> = values
        .iter()
        .map(|(name, totals)| (name, value(totals)))
        .collect();
    rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    eprintln!("  {title}:");
    for (name, count) in rows.into_iter().take(10) {
        eprintln!("    {count}: {name}");
    }
}

fn render_expansion_top_filtered(
    title: &str,
    values: &HashMap<String, ExpansionTotals>,
    include: impl Fn(&str) -> bool,
    value: impl Fn(&ExpansionTotals) -> u64,
) {
    let mut rows: Vec<_> = values
        .iter()
        .filter(|(name, _)| include(name))
        .map(|(name, totals)| (name, value(totals)))
        .collect();
    rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    eprintln!("  {title}:");
    for (name, count) in rows.into_iter().take(10) {
        eprintln!("    {count}: {name}");
    }
}

fn render_count_top(title: &str, values: &HashMap<String, u64>) {
    let mut rows: Vec<_> = values.iter().collect();
    rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    eprintln!("  {title}:");
    for (name, count) in rows.into_iter().take(10) {
        eprintln!("    {count}: {name}");
    }
}

fn render_reason_count_top(
    title: &str,
    values: &HashMap<(DtsExpansionReason, String), u64>,
    reason: DtsExpansionReason,
) {
    let mut rows: Vec<_> = values
        .iter()
        .filter_map(|((entry_reason, name), count)| {
            (*entry_reason == reason).then_some((name, count))
        })
        .collect();
    rows.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    eprintln!("  {title}:");
    for (name, count) in rows.into_iter().take(10) {
        eprintln!("    {count}: {name}");
    }
}
