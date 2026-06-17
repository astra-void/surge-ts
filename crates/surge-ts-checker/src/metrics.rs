//! Instrumentation: program-wide performance counters and phase timings.
//!
//! Pure diagnostics, gated behind `--timings` / `COUNTERS_ENABLED`. Split out of
//! `program.rs` so the checking pipeline reads without the counter plumbing
//! interleaved. Re-exported from `program` for `crate::program::record_*` callers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use surge_ts_types::{snapshot_function_type_counters, snapshot_union_type_counters};

#[derive(Debug, Default, Clone)]
pub(crate) struct ProgramCounters {
    pub(crate) files_total: u64,
    pub(crate) root_source_files: u64,
    pub(crate) dependency_declaration_files: u64,
    pub(crate) generated_default_lib_files: u64,
    pub(crate) parsed_root_source_files: u64,
    pub(crate) parsed_dependency_declaration_files: u64,
    pub(crate) parsed_generated_default_lib_files: u64,
    pub(crate) checker_arena_alloc_count: u64,
    pub(crate) arena_declaration_key_alloc_count: u64,
    pub(crate) arena_type_declaration_payload_alloc_count: u64,
    pub(crate) arena_object_type_payload_alloc_count: u64,
    pub(crate) type_declaration_payload_deep_clone_count: u64,
    pub(crate) type_declaration_header_copy_count: u64,
    pub(crate) object_type_payload_deep_clone_count: u64,
    pub(crate) object_type_alloc_count: u64,
    pub(crate) union_type_alloc_count: u64,
    pub(crate) function_type_alloc_count: u64,
    pub(crate) module_analysis_total_calls: u64,
    pub(crate) module_analysis_unique_files: u64,
    pub(crate) module_analysis_duplicate_calls: u64,
    pub(crate) type_declaration_table_clone_count: u64,
    pub(crate) type_declaration_table_merge_count: u64,
    pub(crate) type_declaration_id_copy_count: u64,
    pub(crate) type_declaration_entries_merged_total: u64,
    pub(crate) generated_default_lib_table_clone_count: u64,
    pub(crate) dependency_declaration_table_clone_count: u64,
    pub(crate) module_scope_cache_hits: u64,
    pub(crate) module_scope_cache_misses: u64,
    pub(crate) declaration_lookup_count: u64,
    pub(crate) declaration_lookup_layer_count_total: u64,
    pub(crate) expression_check_count: u64,
    pub(crate) expression_infer_count: u64,
    pub(crate) assignability_check_count: u64,
    pub(crate) property_lookup_count: u64,
    pub(crate) call_resolution_count: u64,
    pub(crate) generic_call_inference_attempt_count: u64,
    pub(crate) generic_call_inference_success_count: u64,
    pub(crate) generic_call_inference_failed_count: u64,
    pub(crate) generic_call_inference_explicit_type_args_skip_count: u64,
    pub(crate) generic_call_inference_unresolved_argument_skip_count: u64,
    pub(crate) generic_call_inference_tuple_return_suppressed_count: u64,
    pub(crate) generic_call_inference_candidate_count: u64,
    pub(crate) generic_indexed_access_attempt_count: u64,
    pub(crate) generic_indexed_access_substituted_receiver_count: u64,
    pub(crate) generic_indexed_access_substituted_key_count: u64,
    pub(crate) generic_indexed_access_success_count: u64,
    pub(crate) generic_indexed_access_unknown_fallback_count: u64,
    pub(crate) generic_indexed_access_invalid_key_count: u64,
    pub(crate) object_literal_property_check_count: u64,
    pub(crate) function_body_check_count: u64,
    pub(crate) type_declaration_lookup_count: u64,
    pub(crate) type_declaration_lookup_layer_steps_total: u64,
    pub(crate) type_clone_count: u64,
    pub(crate) object_type_clone_count: u64,
    pub(crate) object_type_id_copy_count: u64,
    pub(crate) union_type_clone_count: u64,
    pub(crate) symbol_name_clone_count: u64,
    pub(crate) string_key_clone_count: u64,
    pub(crate) flow_local_name_clone_count: u64,
    pub(crate) string_path_lookup_count: u64,
    pub(crate) canonical_file_id_lookup_count: u64,
    pub(crate) function_type_copy_from_expression_identifier_count: u64,
    pub(crate) function_type_copy_from_expression_call_return_count: u64,
    pub(crate) function_type_copy_from_expression_optional_call_return_count: u64,
    pub(crate) union_type_copy_from_expression_identifier_count: u64,
    pub(crate) union_type_copy_from_expression_call_return_count: u64,
    pub(crate) union_type_copy_from_expression_optional_call_return_count: u64,
    pub(crate) flow_function_count: u64,
    pub(crate) flow_function_skipped_count: u64,
    pub(crate) flow_statement_count: u64,
    pub(crate) flow_expression_visit_count: u64,
    pub(crate) flow_identifier_read_count: u64,
    pub(crate) flow_scope_push_count: u64,
    pub(crate) flow_scope_pop_count: u64,
    pub(crate) flow_future_declaration_collection_count: u64,
    pub(crate) flow_future_declaration_entries_total: u64,
    pub(crate) flow_state_clone_count: u64,
    pub(crate) flow_scope_locals_clone_count: u64,
    pub(crate) flow_state_full_clone_avoided_count: u64,
    pub(crate) flow_branch_merge_count: u64,
    pub(crate) flow_branch_merge_scope_count: u64,
    pub(crate) flow_branch_merge_local_iteration_count: u64,
    pub(crate) flow_branch_merge_fast_path_count: u64,
    pub(crate) flow_branch_empty_delta_count: u64,
    pub(crate) flow_branch_changed_local_count: u64,
    pub(crate) flow_read_lookup_count: u64,
    pub(crate) flow_read_lookup_scope_steps_total: u64,
    pub(crate) flow_return_analysis_walk_count: u64,
    pub(crate) flow_truthiness_check_count: u64,
    pub(crate) type_name_lookup_string_count: u64,
    pub(crate) symbol_info_handle_copy_count: u64,
    pub(crate) symbol_info_payload_deep_clone_count: u64,
    pub(crate) symbol_table_clone_count: u64,
    pub(crate) symbol_table_entry_handle_copy_count: u64,
    pub(crate) scope_stack_visible_rebuild_count: u64,
    pub(crate) scope_stack_visible_symbol_handle_copy_count: u64,
    pub(crate) module_export_table_clone_count: u64,
    pub(crate) module_export_entry_clone_count: u64,
    pub(crate) module_export_symbol_handle_copy_count: u64,
    pub(crate) module_export_borrowed_lookup_count: u64,
    pub(crate) module_export_namespace_export_object_materialization_count: u64,
    pub(crate) module_export_namespace_export_object_property_count: u64,
}

static PROGRAM_COUNTERS: OnceLock<Mutex<ProgramCounters>> = OnceLock::new();

/// Instrumentation counters are pure diagnostics that funnel through a single
/// global `Mutex<ProgramCounters>`. They are only emitted when `--timings` is
/// set, so recording them on the hot path otherwise just serializes every
/// symbol lookup and table clone on one lock. This gate makes the recording a
/// single relaxed atomic load when counters are not being collected.
static COUNTERS_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_counters_enabled(enabled: bool) {
    COUNTERS_ENABLED.store(enabled, Ordering::Relaxed);
}

#[derive(Debug, Default)]
pub(crate) struct ProgramTimings {
    pub(crate) parsing: Duration,
    pub(crate) ambient_collection: Duration,
    pub(crate) dependency_declaration_parse_time: Duration,
    pub(crate) dependency_declaration_lower_time: Duration,
    pub(crate) generated_default_lib_parse_time: Duration,
    pub(crate) generated_default_lib_lower_time: Duration,
    pub(crate) generated_default_lib_global_collection: Duration,
    pub(crate) root_source_global_collection: Duration,
    pub(crate) dependency_declaration_collection: Duration,
    pub(crate) type_declaration_collection: Duration,
    pub(crate) preliminary_module_type_binding_collection: Duration,
    pub(crate) module_analysis_collection: Duration,
    pub(crate) declaration_table_merging_cloning: Duration,
    pub(crate) utility_alias_validation: Duration,
    pub(crate) module_binding: Duration,
    pub(crate) preliminary_export_table_resolution: Duration,
    pub(crate) final_export_table_resolution: Duration,
    pub(crate) import_binding_resolution: Duration,
    pub(crate) import_specifier_resolution: Duration,
    pub(crate) export_table_lookup: Duration,
    pub(crate) re_export_expansion: Duration,
    pub(crate) package_export_lookup: Duration,
    pub(crate) type_binding_insert: Duration,
    pub(crate) value_binding_insert: Duration,
    pub(crate) module_resolution_scope_construction: Duration,
    pub(crate) ambient_module_binding: Duration,
    pub(crate) re_export_resolution: Duration,
    pub(crate) package_dependency_module_binding: Duration,
    pub(crate) generated_default_lib_module_handling: Duration,
    pub(crate) clone_copy_heavy_operations: Duration,
    pub(crate) declaration_validation: Duration,
    pub(crate) per_file_statement_checking: Duration,
    pub(crate) variable_declaration_checking: Duration,
    pub(crate) function_declaration_checking: Duration,
    pub(crate) expression_statement_checking: Duration,
    pub(crate) return_statement_checking: Duration,
    pub(crate) call_expression_checking: Duration,
    pub(crate) object_literal_checking: Duration,
    pub(crate) property_access_checking: Duration,
    pub(crate) assignability_checking: Duration,
    pub(crate) type_inference: Duration,
    pub(crate) flow_narrowing: Duration,
    pub(crate) file_metrics: HashMap<String, FileTimings>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FileTimings {
    pub(crate) collect_type_declarations_passes: u64,
    pub(crate) lowered_type_declarations: u64,
    pub(crate) validate_local_type_declarations_passes: u64,
    pub(crate) validate_local_type_declarations_items: u64,
    pub(crate) collect_type_declarations_duration: Duration,
    pub(crate) validate_local_type_declarations_duration: Duration,
}

pub(crate) fn record_program_timing(
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    update: impl FnOnce(&mut ProgramTimings),
) {
    let Some(timings) = timings else {
        return;
    };

    if let Ok(mut guard) = timings.lock() {
        update(&mut guard);
    }
}

pub(crate) fn record_program_file_timing(
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    file_name: &str,
    update: impl FnOnce(&mut FileTimings),
) {
    let Some(timings) = timings else {
        return;
    };

    if let Ok(mut guard) = timings.lock() {
        update(guard.file_metrics.entry(file_name.to_string()).or_default());
    }
}

pub(crate) fn render_program_timings(timings: &Arc<Mutex<ProgramTimings>>) {
    let Ok(timings) = timings.lock() else {
        return;
    };

    eprintln!("Timings:");
    eprintln!("  parsing: {}", format_duration(timings.parsing));
    eprintln!(
        "  ambient_collection: {}",
        format_duration(timings.ambient_collection)
    );
    eprintln!(
        "  dependency_declaration_parse_time: {}",
        format_duration(timings.dependency_declaration_parse_time)
    );
    eprintln!(
        "  dependency_declaration_lower_time: {}",
        format_duration(timings.dependency_declaration_lower_time)
    );
    eprintln!(
        "  generated_default_lib_parse_time: {}",
        format_duration(timings.generated_default_lib_parse_time)
    );
    eprintln!(
        "  generated_default_lib_lower_time: {}",
        format_duration(timings.generated_default_lib_lower_time)
    );
    eprintln!(
        "  generated_default_lib_global_collection: {}",
        format_duration(timings.generated_default_lib_global_collection)
    );
    eprintln!(
        "  root_source_global_collection: {}",
        format_duration(timings.root_source_global_collection)
    );
    eprintln!(
        "  dependency_declaration_collection: {}",
        format_duration(timings.dependency_declaration_collection)
    );
    eprintln!(
        "  type_declaration_collection: {}",
        format_duration(timings.type_declaration_collection)
    );
    eprintln!(
        "    preliminary_module_type_binding_collection: {}",
        format_duration(timings.preliminary_module_type_binding_collection)
    );
    eprintln!(
        "    module_analysis_collection: {}",
        format_duration(timings.module_analysis_collection)
    );
    eprintln!(
        "    declaration_table_merging_cloning: {}",
        format_duration(timings.declaration_table_merging_cloning)
    );
    eprintln!(
        "    utility_alias_validation: {}",
        format_duration(timings.utility_alias_validation)
    );
    eprintln!(
        "  module_binding: {}",
        format_duration(timings.module_binding)
    );
    eprintln!(
        "    preliminary_export_table_resolution: {}",
        format_duration(timings.preliminary_export_table_resolution)
    );
    eprintln!(
        "    final_export_table_resolution: {}",
        format_duration(timings.final_export_table_resolution)
    );
    eprintln!(
        "    import_binding_resolution: {}",
        format_duration(timings.import_binding_resolution)
    );
    eprintln!(
        "      import_specifier_resolution: {}",
        format_duration(timings.import_specifier_resolution)
    );
    eprintln!(
        "      export_table_lookup: {}",
        format_duration(timings.export_table_lookup)
    );
    eprintln!(
        "      re_export_expansion: {}",
        format_duration(timings.re_export_expansion)
    );
    eprintln!(
        "      package_export_lookup: {}",
        format_duration(timings.package_export_lookup)
    );
    eprintln!(
        "      type_binding_insert: {}",
        format_duration(timings.type_binding_insert)
    );
    eprintln!(
        "      value_binding_insert: {}",
        format_duration(timings.value_binding_insert)
    );
    eprintln!(
        "    module_resolution_scope_construction: {}",
        format_duration(timings.module_resolution_scope_construction)
    );
    eprintln!(
        "    ambient_module_binding: {}",
        format_duration(timings.ambient_module_binding)
    );
    eprintln!(
        "    re_export_resolution: {}",
        format_duration(timings.re_export_resolution)
    );
    eprintln!(
        "    package_dependency_module_binding: {}",
        format_duration(timings.package_dependency_module_binding)
    );
    eprintln!(
        "    generated_default_lib_module_handling: {}",
        format_duration(timings.generated_default_lib_module_handling)
    );
    eprintln!(
        "    clone_copy_heavy_operations: {}",
        format_duration(timings.clone_copy_heavy_operations)
    );
    eprintln!(
        "  declaration_validation: {}",
        format_duration(timings.declaration_validation)
    );
    eprintln!(
        "  per_file_statement_checking: {}",
        format_duration(timings.per_file_statement_checking)
    );
    eprintln!(
        "    variable_declaration_checking: {}",
        format_duration(timings.variable_declaration_checking)
    );
    eprintln!(
        "    function_declaration_checking: {}",
        format_duration(timings.function_declaration_checking)
    );
    eprintln!(
        "    expression_statement_checking: {}",
        format_duration(timings.expression_statement_checking)
    );
    eprintln!(
        "    return_statement_checking: {}",
        format_duration(timings.return_statement_checking)
    );
    eprintln!(
        "    call_expression_checking: {}",
        format_duration(timings.call_expression_checking)
    );
    eprintln!(
        "    object_literal_checking: {}",
        format_duration(timings.object_literal_checking)
    );
    eprintln!(
        "    property_access_checking: {}",
        format_duration(timings.property_access_checking)
    );
    eprintln!(
        "    assignability_checking: {}",
        format_duration(timings.assignability_checking)
    );
    eprintln!(
        "    type_inference: {}",
        format_duration(timings.type_inference)
    );
    eprintln!(
        "    flow_narrowing: {}",
        format_duration(timings.flow_narrowing)
    );
    if !timings.file_metrics.is_empty() {
        let mut file_metrics = timings.file_metrics.iter().collect::<Vec<_>>();
        file_metrics.sort_by(|(file_a, metrics_a), (file_b, metrics_b)| {
            metrics_b
                .collect_type_declarations_passes
                .cmp(&metrics_a.collect_type_declarations_passes)
                .then_with(|| file_a.cmp(file_b))
        });

        eprintln!("  file_metrics:");
        for (file_name, metrics) in file_metrics {
            eprintln!(
                "    {} | collect_type_declarations={} lower_type_declarations={} validate_local_type_declarations={} | collect_time={} validate_time={}",
                file_name,
                metrics.collect_type_declarations_passes,
                metrics.lowered_type_declarations,
                metrics.validate_local_type_declarations_passes,
                format_duration(metrics.collect_type_declarations_duration),
                format_duration(metrics.validate_local_type_declarations_duration)
            );
        }
    }

    let counters = snapshot_program_counters();
    eprintln!("  counters:");
    eprintln!("    files_total: {}", counters.files_total);
    eprintln!("    root_source_files: {}", counters.root_source_files);
    eprintln!(
        "    dependency_declaration_files: {}",
        counters.dependency_declaration_files
    );
    eprintln!(
        "    generated_default_lib_files: {}",
        counters.generated_default_lib_files
    );
    eprintln!(
        "    parsed_root_source_files: {}",
        counters.parsed_root_source_files
    );
    eprintln!(
        "    parsed_dependency_declaration_files: {}",
        counters.parsed_dependency_declaration_files
    );
    eprintln!(
        "    parsed_generated_default_lib_files: {}",
        counters.parsed_generated_default_lib_files
    );
    eprintln!(
        "    checker_arena_alloc_count: {}",
        counters.checker_arena_alloc_count
    );
    eprintln!(
        "    arena_declaration_key_alloc_count: {}",
        counters.arena_declaration_key_alloc_count
    );
    eprintln!(
        "    arena_type_declaration_payload_alloc_count: {}",
        counters.arena_type_declaration_payload_alloc_count
    );
    eprintln!(
        "    arena_object_type_payload_alloc_count: {}",
        counters.arena_object_type_payload_alloc_count
    );
    eprintln!(
        "    type_declaration_payload_deep_clone_count: {}",
        counters.type_declaration_payload_deep_clone_count
    );
    eprintln!(
        "    type_declaration_header_copy_count: {}",
        counters.type_declaration_header_copy_count
    );
    eprintln!(
        "    object_type_payload_deep_clone_count: {}",
        counters.object_type_payload_deep_clone_count
    );
    eprintln!(
        "    object_type_alloc_count: {}",
        counters.object_type_alloc_count
    );
    eprintln!(
        "    union_type_alloc_count: {}",
        counters.union_type_alloc_count
    );
    eprintln!(
        "    function_type_alloc_count: {}",
        counters.function_type_alloc_count
    );
    eprintln!(
        "    module_analysis_total_calls: {}",
        counters.module_analysis_total_calls
    );
    eprintln!(
        "    module_analysis_unique_files: {}",
        counters.module_analysis_unique_files
    );
    eprintln!(
        "    module_analysis_duplicate_calls: {}",
        counters.module_analysis_duplicate_calls
    );
    eprintln!(
        "    type_declaration_table_clone_count: {}",
        counters.type_declaration_table_clone_count
    );
    eprintln!(
        "    type_declaration_table_merge_count: {}",
        counters.type_declaration_table_merge_count
    );
    eprintln!(
        "    type_declaration_id_copy_count: {}",
        counters.type_declaration_id_copy_count
    );
    eprintln!(
        "    type_declaration_entries_merged_total: {}",
        counters.type_declaration_entries_merged_total
    );
    eprintln!(
        "    generated_default_lib_table_clone_count: {}",
        counters.generated_default_lib_table_clone_count
    );
    eprintln!(
        "    dependency_declaration_table_clone_count: {}",
        counters.dependency_declaration_table_clone_count
    );
    eprintln!(
        "    module_scope_cache_hits: {}",
        counters.module_scope_cache_hits
    );
    eprintln!(
        "    module_scope_cache_misses: {}",
        counters.module_scope_cache_misses
    );
    eprintln!(
        "    declaration_lookup_count: {}",
        counters.declaration_lookup_count
    );
    let declaration_lookup_avg = if counters.declaration_lookup_count == 0 {
        0.0
    } else {
        counters.declaration_lookup_layer_count_total as f64
            / counters.declaration_lookup_count as f64
    };
    eprintln!(
        "    declaration_lookup_layer_count_avg: {:.2}",
        declaration_lookup_avg
    );
    eprintln!(
        "    expression_check_count: {}",
        counters.expression_check_count
    );
    eprintln!(
        "    expression_infer_count: {}",
        counters.expression_infer_count
    );
    eprintln!(
        "    assignability_check_count: {}",
        counters.assignability_check_count
    );
    eprintln!(
        "    property_lookup_count: {}",
        counters.property_lookup_count
    );
    eprintln!(
        "    call_resolution_count: {}",
        counters.call_resolution_count
    );
    eprintln!(
        "    generic_call_inference_attempt_count: {}",
        counters.generic_call_inference_attempt_count
    );
    eprintln!(
        "    generic_call_inference_success_count: {}",
        counters.generic_call_inference_success_count
    );
    eprintln!(
        "    generic_call_inference_failed_count: {}",
        counters.generic_call_inference_failed_count
    );
    eprintln!(
        "    generic_call_inference_explicit_type_args_skip_count: {}",
        counters.generic_call_inference_explicit_type_args_skip_count
    );
    eprintln!(
        "    generic_call_inference_unresolved_argument_skip_count: {}",
        counters.generic_call_inference_unresolved_argument_skip_count
    );
    eprintln!(
        "    generic_call_inference_tuple_return_suppressed_count: {}",
        counters.generic_call_inference_tuple_return_suppressed_count
    );
    eprintln!(
        "    generic_call_inference_candidate_count: {}",
        counters.generic_call_inference_candidate_count
    );
    eprintln!(
        "    generic_indexed_access_attempt_count: {}",
        counters.generic_indexed_access_attempt_count
    );
    eprintln!(
        "    generic_indexed_access_substituted_receiver_count: {}",
        counters.generic_indexed_access_substituted_receiver_count
    );
    eprintln!(
        "    generic_indexed_access_substituted_key_count: {}",
        counters.generic_indexed_access_substituted_key_count
    );
    eprintln!(
        "    generic_indexed_access_success_count: {}",
        counters.generic_indexed_access_success_count
    );
    eprintln!(
        "    generic_indexed_access_unknown_fallback_count: {}",
        counters.generic_indexed_access_unknown_fallback_count
    );
    eprintln!(
        "    generic_indexed_access_invalid_key_count: {}",
        counters.generic_indexed_access_invalid_key_count
    );
    eprintln!(
        "    object_literal_property_check_count: {}",
        counters.object_literal_property_check_count
    );
    eprintln!(
        "    function_body_check_count: {}",
        counters.function_body_check_count
    );
    eprintln!(
        "    type_declaration_lookup_count: {}",
        counters.type_declaration_lookup_count
    );
    eprintln!(
        "    type_declaration_lookup_layer_steps_total: {}",
        counters.type_declaration_lookup_layer_steps_total
    );
    eprintln!("    type_clone_count: {}", counters.type_clone_count);
    eprintln!(
        "    object_type_clone_count: {}",
        counters.object_type_clone_count
    );
    eprintln!(
        "    object_type_id_copy_count: {}",
        counters.object_type_id_copy_count
    );
    eprintln!(
        "    union_type_clone_count: {}",
        counters.union_type_clone_count
    );
    eprintln!(
        "    symbol_name_clone_count: {}",
        counters.symbol_name_clone_count
    );
    eprintln!(
        "    string_key_clone_count: {}",
        counters.string_key_clone_count
    );
    eprintln!(
        "    flow_local_name_clone_count: {}",
        counters.flow_local_name_clone_count
    );
    eprintln!(
        "    string_path_lookup_count: {}",
        counters.string_path_lookup_count
    );
    eprintln!(
        "    canonical_file_id_lookup_count: {}",
        counters.canonical_file_id_lookup_count
    );
    eprintln!("    flow_function_count: {}", counters.flow_function_count);
    eprintln!(
        "    flow_function_skipped_count: {}",
        counters.flow_function_skipped_count
    );
    eprintln!(
        "    flow_statement_count: {}",
        counters.flow_statement_count
    );
    eprintln!(
        "    flow_expression_visit_count: {}",
        counters.flow_expression_visit_count
    );
    eprintln!(
        "    flow_identifier_read_count: {}",
        counters.flow_identifier_read_count
    );
    eprintln!(
        "    flow_scope_push_count: {}",
        counters.flow_scope_push_count
    );
    eprintln!(
        "    flow_scope_pop_count: {}",
        counters.flow_scope_pop_count
    );
    eprintln!(
        "    flow_future_declaration_collection_count: {}",
        counters.flow_future_declaration_collection_count
    );
    eprintln!(
        "    flow_future_declaration_entries_total: {}",
        counters.flow_future_declaration_entries_total
    );
    eprintln!(
        "    flow_state_clone_count: {}",
        counters.flow_state_clone_count
    );
    eprintln!(
        "    flow_scope_locals_clone_count: {}",
        counters.flow_scope_locals_clone_count
    );
    eprintln!(
        "    flow_state_full_clone_avoided_count: {}",
        counters.flow_state_full_clone_avoided_count
    );
    eprintln!(
        "    flow_branch_merge_count: {}",
        counters.flow_branch_merge_count
    );
    eprintln!(
        "    flow_branch_merge_scope_count: {}",
        counters.flow_branch_merge_scope_count
    );
    eprintln!(
        "    flow_branch_merge_local_iteration_count: {}",
        counters.flow_branch_merge_local_iteration_count
    );
    eprintln!(
        "    flow_branch_merge_fast_path_count: {}",
        counters.flow_branch_merge_fast_path_count
    );
    eprintln!(
        "    flow_branch_empty_delta_count: {}",
        counters.flow_branch_empty_delta_count
    );
    eprintln!(
        "    flow_branch_changed_local_count: {}",
        counters.flow_branch_changed_local_count
    );
    eprintln!(
        "    flow_read_lookup_count: {}",
        counters.flow_read_lookup_count
    );
    eprintln!(
        "    flow_read_lookup_scope_steps_total: {}",
        counters.flow_read_lookup_scope_steps_total
    );
    eprintln!(
        "    flow_return_analysis_walk_count: {}",
        counters.flow_return_analysis_walk_count
    );
    eprintln!(
        "    flow_truthiness_check_count: {}",
        counters.flow_truthiness_check_count
    );
    eprintln!(
        "    type_name_lookup_string_count: {}",
        counters.type_name_lookup_string_count
    );
    eprintln!(
        "    symbol_info_handle_copy_count: {}",
        counters.symbol_info_handle_copy_count
    );
    eprintln!(
        "    symbol_info_payload_deep_clone_count: {}",
        counters.symbol_info_payload_deep_clone_count
    );
    eprintln!(
        "    symbol_table_clone_count: {}",
        counters.symbol_table_clone_count
    );
    eprintln!(
        "    symbol_table_entry_handle_copy_count: {}",
        counters.symbol_table_entry_handle_copy_count
    );
    eprintln!(
        "    scope_stack_visible_rebuild_count: {}",
        counters.scope_stack_visible_rebuild_count
    );
    eprintln!(
        "    scope_stack_visible_symbol_handle_copy_count: {}",
        counters.scope_stack_visible_symbol_handle_copy_count
    );
    eprintln!(
        "    module_export_table_clone_count: {}",
        counters.module_export_table_clone_count
    );
    eprintln!(
        "    module_export_entry_clone_count: {}",
        counters.module_export_entry_clone_count
    );
    eprintln!(
        "    module_export_symbol_handle_copy_count: {}",
        counters.module_export_symbol_handle_copy_count
    );
    eprintln!(
        "    module_export_borrowed_lookup_count: {}",
        counters.module_export_borrowed_lookup_count
    );
    eprintln!(
        "    module_export_namespace_export_object_materialization_count: {}",
        counters.module_export_namespace_export_object_materialization_count
    );
    eprintln!(
        "    module_export_namespace_export_object_property_count: {}",
        counters.module_export_namespace_export_object_property_count
    );
    eprintln!(
        "    function_type_copy_from_expression_identifier_count: {}",
        counters.function_type_copy_from_expression_identifier_count
    );
    eprintln!(
        "    function_type_copy_from_expression_call_return_count: {}",
        counters.function_type_copy_from_expression_call_return_count
    );
    eprintln!(
        "    function_type_copy_from_expression_optional_call_return_count: {}",
        counters.function_type_copy_from_expression_optional_call_return_count
    );
    eprintln!(
        "    union_type_copy_from_expression_identifier_count: {}",
        counters.union_type_copy_from_expression_identifier_count
    );
    eprintln!(
        "    union_type_copy_from_expression_call_return_count: {}",
        counters.union_type_copy_from_expression_call_return_count
    );
    eprintln!(
        "    union_type_copy_from_expression_optional_call_return_count: {}",
        counters.union_type_copy_from_expression_optional_call_return_count
    );

    let function_type_counters = snapshot_function_type_counters();
    eprintln!(
        "    function_type_payload_alloc_count: {}",
        function_type_counters.function_type_payload_alloc_count
    );
    eprintln!(
        "    function_type_payload_deep_clone_count: {}",
        function_type_counters.function_type_payload_deep_clone_count
    );
    eprintln!(
        "    function_type_handle_copy_count: {}",
        function_type_counters.function_type_handle_copy_count
    );
    eprintln!(
        "    function_type_clone_count: {}",
        function_type_counters.function_type_clone_count
    );
    eprintln!(
        "    function_type_copy_from_expression_inference_count: {}",
        function_type_counters.function_type_copy_from_expression_inference_count
    );
    eprintln!(
        "    function_type_copy_from_call_resolution_count: {}",
        function_type_counters.function_type_copy_from_call_resolution_count
    );
    eprintln!(
        "    function_type_copy_from_property_call_resolution_count: {}",
        function_type_counters.function_type_copy_from_property_call_resolution_count
    );
    eprintln!(
        "    function_type_copy_from_function_body_setup_count: {}",
        function_type_counters.function_type_copy_from_function_body_setup_count
    );
    eprintln!(
        "    function_type_copy_from_return_checking_count: {}",
        function_type_counters.function_type_copy_from_return_checking_count
    );
    eprintln!(
        "    function_type_copy_from_expected_type_count: {}",
        function_type_counters.function_type_copy_from_expected_type_count
    );
    eprintln!(
        "    function_type_copy_from_symbol_table_count: {}",
        function_type_counters.function_type_copy_from_symbol_table_count
    );
    eprintln!(
        "    function_type_copy_from_module_export_count: {}",
        function_type_counters.function_type_copy_from_module_export_count
    );
    eprintln!(
        "    function_type_copy_from_scope_or_context_count: {}",
        function_type_counters.function_type_copy_from_scope_or_context_count
    );
    eprintln!(
        "    function_type_copy_from_substitution_unchanged_count: {}",
        function_type_counters.function_type_copy_from_substitution_unchanged_count
    );
    eprintln!(
        "    function_type_copy_from_substitution_changed_count: {}",
        function_type_counters.function_type_copy_from_substitution_changed_count
    );
    eprintln!(
        "    function_type_copy_from_diagnostic_formatting_count: {}",
        function_type_counters.function_type_copy_from_diagnostic_formatting_count
    );
    eprintln!(
        "    function_type_copy_unattributed_count: {}",
        function_type_counters.function_type_copy_unattributed_count
    );

    let union_type_counters = snapshot_union_type_counters();
    eprintln!(
        "    union_type_payload_alloc_count: {}",
        union_type_counters.union_type_payload_alloc_count
    );
    eprintln!(
        "    union_type_payload_deep_clone_count: {}",
        union_type_counters.union_type_payload_deep_clone_count
    );
    eprintln!(
        "    union_type_handle_copy_count: {}",
        union_type_counters.union_type_handle_copy_count
    );
    eprintln!(
        "    union_type_copy_from_expression_inference_count: {}",
        union_type_counters.union_type_copy_from_expression_inference_count
    );
    eprintln!(
        "    union_type_copy_from_call_resolution_count: {}",
        union_type_counters.union_type_copy_from_call_resolution_count
    );
    eprintln!(
        "    union_type_copy_from_property_call_resolution_count: {}",
        union_type_counters.union_type_copy_from_property_call_resolution_count
    );
    eprintln!(
        "    union_type_copy_from_function_body_setup_count: {}",
        union_type_counters.union_type_copy_from_function_body_setup_count
    );
    eprintln!(
        "    union_type_copy_from_return_checking_count: {}",
        union_type_counters.union_type_copy_from_return_checking_count
    );
    eprintln!(
        "    union_type_copy_from_expected_type_count: {}",
        union_type_counters.union_type_copy_from_expected_type_count
    );
    eprintln!(
        "    union_type_copy_from_symbol_table_count: {}",
        union_type_counters.union_type_copy_from_symbol_table_count
    );
    eprintln!(
        "    union_type_copy_from_module_export_count: {}",
        union_type_counters.union_type_copy_from_module_export_count
    );
    eprintln!(
        "    union_type_copy_from_scope_or_context_count: {}",
        union_type_counters.union_type_copy_from_scope_or_context_count
    );
    eprintln!(
        "    union_type_copy_from_substitution_unchanged_count: {}",
        union_type_counters.union_type_copy_from_substitution_unchanged_count
    );
    eprintln!(
        "    union_type_copy_from_substitution_changed_count: {}",
        union_type_counters.union_type_copy_from_substitution_changed_count
    );
    eprintln!(
        "    union_type_copy_from_diagnostic_formatting_count: {}",
        union_type_counters.union_type_copy_from_diagnostic_formatting_count
    );
    eprintln!(
        "    union_type_copy_unattributed_count: {}",
        union_type_counters.union_type_copy_unattributed_count
    );
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum TableCloneKind {
    General,
    GeneratedDefaultLib,
    DependencyDeclaration,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TableMergeKind {
    General,
}

pub(crate) fn record_string_path_lookup() {
    record_program_counter(|c| c.string_path_lookup_count += 1);
}

pub(crate) fn record_canonical_file_id_lookup() {
    record_program_counter(|c| c.canonical_file_id_lookup_count += 1);
}

pub(crate) fn record_declaration_lookup(layer_count: usize) {
    record_program_counter(|c| {
        c.declaration_lookup_count += 1;
        c.declaration_lookup_layer_count_total += layer_count as u64;
    });
}

pub(crate) fn record_expression_check() {
    record_program_counter(|c| c.expression_check_count += 1);
}

pub(crate) fn record_expression_infer() {
    record_program_counter(|c| c.expression_infer_count += 1);
}

pub(crate) fn record_assignability_check() {
    record_program_counter(|c| c.assignability_check_count += 1);
}

pub(crate) fn record_property_lookup() {
    record_program_counter(|c| c.property_lookup_count += 1);
}

pub(crate) fn record_call_resolution() {
    record_program_counter(|c| c.call_resolution_count += 1);
}

pub(crate) fn record_generic_call_inference_attempt() {
    record_program_counter(|c| c.generic_call_inference_attempt_count += 1);
}

pub(crate) fn record_generic_call_inference_success() {
    record_program_counter(|c| c.generic_call_inference_success_count += 1);
}

pub(crate) fn record_generic_call_inference_failed() {
    record_program_counter(|c| c.generic_call_inference_failed_count += 1);
}

pub(crate) fn record_generic_call_inference_explicit_type_args_skip() {
    record_program_counter(|c| c.generic_call_inference_explicit_type_args_skip_count += 1);
}

pub(crate) fn record_generic_call_inference_unresolved_argument_skip() {
    record_program_counter(|c| c.generic_call_inference_unresolved_argument_skip_count += 1);
}

pub(crate) fn record_generic_call_inference_tuple_return_suppressed() {
    record_program_counter(|c| c.generic_call_inference_tuple_return_suppressed_count += 1);
}

pub(crate) fn record_generic_call_inference_candidate() {
    record_program_counter(|c| c.generic_call_inference_candidate_count += 1);
}

pub(crate) fn record_generic_indexed_access_attempt() {
    record_program_counter(|c| c.generic_indexed_access_attempt_count += 1);
}

pub(crate) fn record_generic_indexed_access_substituted_receiver() {
    record_program_counter(|c| c.generic_indexed_access_substituted_receiver_count += 1);
}

pub(crate) fn record_generic_indexed_access_substituted_key() {
    record_program_counter(|c| c.generic_indexed_access_substituted_key_count += 1);
}

pub(crate) fn record_generic_indexed_access_success() {
    record_program_counter(|c| c.generic_indexed_access_success_count += 1);
}

pub(crate) fn record_generic_indexed_access_unknown_fallback() {
    record_program_counter(|c| c.generic_indexed_access_unknown_fallback_count += 1);
}

pub(crate) fn record_generic_indexed_access_invalid_key() {
    record_program_counter(|c| c.generic_indexed_access_invalid_key_count += 1);
}

pub(crate) fn record_object_literal_property_check() {
    record_program_counter(|c| c.object_literal_property_check_count += 1);
}

pub(crate) fn record_function_body_check() {
    record_program_counter(|c| c.function_body_check_count += 1);
}

pub(crate) fn record_type_declaration_lookup(layer_steps: usize) {
    record_program_counter(|c| {
        c.type_declaration_lookup_count += 1;
        c.type_declaration_lookup_layer_steps_total += layer_steps as u64;
    });
}

pub(crate) fn record_module_scope_cache_hit() {
    record_program_counter(|c| c.module_scope_cache_hits += 1);
}

pub(crate) fn record_module_scope_cache_miss() {
    record_program_counter(|c| c.module_scope_cache_misses += 1);
}

pub(crate) fn record_type_clone_count() {
    record_program_counter(|c| c.type_clone_count += 1);
}

pub(crate) fn record_checker_arena_alloc_count() {
    record_program_counter(|c| c.checker_arena_alloc_count += 1);
}

pub(crate) fn record_arena_declaration_key_alloc_count() {
    record_program_counter(|c| c.arena_declaration_key_alloc_count += 1);
}

pub(crate) fn record_arena_type_declaration_payload_alloc_count() {
    record_program_counter(|c| c.arena_type_declaration_payload_alloc_count += 1);
}

pub(crate) fn record_arena_object_type_payload_alloc_count() {
    record_program_counter(|c| c.arena_object_type_payload_alloc_count += 1);
}

pub(crate) fn record_type_declaration_payload_deep_clone_count() {
    record_program_counter(|c| c.type_declaration_payload_deep_clone_count += 1);
}

pub(crate) fn record_type_declaration_header_copy_count() {
    record_program_counter(|c| c.type_declaration_header_copy_count += 1);
}

#[allow(dead_code)]
pub(crate) fn record_object_type_payload_deep_clone_count() {
    record_program_counter(|c| c.object_type_payload_deep_clone_count += 1);
}

pub(crate) fn record_object_type_clone_count() {
    record_program_counter(|c| c.object_type_clone_count += 1);
}

pub(crate) fn record_object_type_id_copy_count() {
    record_program_counter(|c| c.object_type_id_copy_count += 1);
}

pub(crate) fn record_union_type_clone_count() {
    record_program_counter(|c| c.union_type_clone_count += 1);
}

#[allow(dead_code)]
pub(crate) fn record_symbol_name_clone_count(count: usize) {
    record_program_counter(|c| c.symbol_name_clone_count += count as u64);
}

#[allow(dead_code)]
pub(crate) fn record_string_key_clone_count(count: usize) {
    record_program_counter(|c| c.string_key_clone_count += count as u64);
}

#[allow(dead_code)]
pub(crate) fn record_flow_local_name_clone_count(count: usize) {
    record_program_counter(|c| c.flow_local_name_clone_count += count as u64);
}

pub(crate) fn record_flow_function_count() {
    record_program_counter(|c| c.flow_function_count += 1);
}

pub(crate) fn record_flow_function_skipped_count() {
    record_program_counter(|c| c.flow_function_skipped_count += 1);
}

pub(crate) fn record_flow_statement_count() {
    record_program_counter(|c| c.flow_statement_count += 1);
}

pub(crate) fn record_flow_expression_visit_count() {
    record_program_counter(|c| c.flow_expression_visit_count += 1);
}

pub(crate) fn record_flow_identifier_read_count() {
    record_program_counter(|c| c.flow_identifier_read_count += 1);
}

pub(crate) fn record_flow_scope_push_count() {
    record_program_counter(|c| c.flow_scope_push_count += 1);
}

pub(crate) fn record_flow_scope_pop_count() {
    record_program_counter(|c| c.flow_scope_pop_count += 1);
}

pub(crate) fn record_flow_future_declaration_collection_count(entry_count: usize) {
    record_program_counter(|c| {
        c.flow_future_declaration_collection_count += 1;
        c.flow_future_declaration_entries_total += entry_count as u64;
    });
}

pub(crate) fn record_flow_state_clone_count(local_count: usize) {
    record_program_counter(|c| {
        c.flow_state_clone_count += 1;
        c.flow_scope_locals_clone_count += local_count as u64;
    });
}

pub(crate) fn record_flow_state_full_clone_avoided_count() {
    record_program_counter(|c| c.flow_state_full_clone_avoided_count += 1);
}

pub(crate) fn record_flow_branch_merge_count(scope_count: usize) {
    record_program_counter(|c| {
        c.flow_branch_merge_count += 1;
        c.flow_branch_merge_scope_count += scope_count as u64;
    });
}

pub(crate) fn record_flow_branch_merge_local_iteration_count(count: usize) {
    record_program_counter(|c| {
        c.flow_branch_merge_local_iteration_count += count as u64;
    });
}

pub(crate) fn record_flow_branch_merge_fast_path_count() {
    record_program_counter(|c| c.flow_branch_merge_fast_path_count += 1);
}

pub(crate) fn record_flow_branch_empty_delta_count() {
    record_program_counter(|c| c.flow_branch_empty_delta_count += 1);
}

pub(crate) fn record_flow_branch_changed_local_count(count: usize) {
    record_program_counter(|c| {
        c.flow_branch_changed_local_count += count as u64;
    });
}

pub(crate) fn record_flow_read_lookup_count(scope_steps: usize) {
    record_program_counter(|c| {
        c.flow_read_lookup_count += 1;
        c.flow_read_lookup_scope_steps_total += scope_steps as u64;
    });
}

pub(crate) fn record_flow_return_analysis_walk_count() {
    record_program_counter(|c| c.flow_return_analysis_walk_count += 1);
}

pub(crate) fn record_flow_truthiness_check_count() {
    record_program_counter(|c| c.flow_truthiness_check_count += 1);
}

pub(crate) fn record_type_name_lookup_string_count(count: usize) {
    record_program_counter(|c| c.type_name_lookup_string_count += count as u64);
}

pub(crate) fn record_symbol_info_handle_copy_count(count: u64) {
    record_program_counter(|c| c.symbol_info_handle_copy_count += count);
}

pub(crate) fn record_symbol_info_payload_deep_clone_count() {
    record_program_counter(|c| c.symbol_info_payload_deep_clone_count += 1);
}

pub(crate) fn record_symbol_table_clone_count() {
    record_program_counter(|c| c.symbol_table_clone_count += 1);
}

pub(crate) fn record_symbol_table_entry_handle_copy_count(count: u64) {
    record_program_counter(|c| c.symbol_table_entry_handle_copy_count += count);
}

#[allow(dead_code)]
pub(crate) fn record_scope_stack_visible_rebuild_count() {
    record_program_counter(|c| c.scope_stack_visible_rebuild_count += 1);
}

pub(crate) fn record_scope_stack_visible_symbol_handle_copy_count(count: u64) {
    record_program_counter(|c| c.scope_stack_visible_symbol_handle_copy_count += count);
}

pub(crate) fn record_function_type_copy_from_expression_identifier_count() {
    record_program_counter(|c| c.function_type_copy_from_expression_identifier_count += 1);
}

pub(crate) fn record_function_type_copy_from_expression_call_return_count() {
    record_program_counter(|c| c.function_type_copy_from_expression_call_return_count += 1);
}

pub(crate) fn record_function_type_copy_from_expression_optional_call_return_count() {
    record_program_counter(|c| {
        c.function_type_copy_from_expression_optional_call_return_count += 1
    });
}

pub(crate) fn record_union_type_copy_from_expression_identifier_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_identifier_count += 1);
}

pub(crate) fn record_union_type_copy_from_expression_call_return_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_call_return_count += 1);
}

pub(crate) fn record_union_type_copy_from_expression_optional_call_return_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_optional_call_return_count += 1);
}

pub(crate) fn record_type_declaration_table_clone(
    _timings: Option<&Arc<Mutex<ProgramTimings>>>,
    entry_count: usize,
    kind: TableCloneKind,
) {
    record_program_counter(|c| {
        c.type_declaration_table_clone_count += 1;
        c.type_declaration_id_copy_count += entry_count as u64;
        match kind {
            TableCloneKind::General => {}
            TableCloneKind::GeneratedDefaultLib => {
                c.generated_default_lib_table_clone_count += 1;
            }
            TableCloneKind::DependencyDeclaration => {
                c.dependency_declaration_table_clone_count += 1;
            }
        }
    });
}

pub(crate) fn record_type_declaration_table_merge(
    _timings: Option<&Arc<Mutex<ProgramTimings>>>,
    entry_count: usize,
    _kind: TableMergeKind,
) {
    record_program_counter(|c| {
        c.type_declaration_table_merge_count += 1;
        c.type_declaration_entries_merged_total += entry_count as u64;
    });
}

pub(crate) fn record_program_counter(update: impl FnOnce(&mut ProgramCounters)) {
    if !COUNTERS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    if let Ok(mut guard) = lock.lock() {
        update(&mut guard);
    }
}

pub(crate) fn record_module_export_table_clone_count() {
    record_program_counter(|c| c.module_export_table_clone_count += 1);
}

pub(crate) fn record_module_export_entry_clone_count(count: u64) {
    record_program_counter(|c| c.module_export_entry_clone_count += count);
}

pub(crate) fn record_module_export_symbol_handle_copy_count(count: u64) {
    record_program_counter(|c| c.module_export_symbol_handle_copy_count += count);
}

pub(crate) fn record_module_export_borrowed_lookup_count() {
    record_program_counter(|c| c.module_export_borrowed_lookup_count += 1);
}

pub(crate) fn record_module_export_namespace_export_object_materialization_count() {
    record_program_counter(|c| c.module_export_namespace_export_object_materialization_count += 1);
}

pub(crate) fn record_module_export_namespace_export_object_property_count(count: u64) {
    record_program_counter(|c| c.module_export_namespace_export_object_property_count += count);
}

pub(crate) fn reset_program_counters() {
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    if let Ok(mut guard) = lock.lock() {
        *guard = ProgramCounters::default();
    }
}

pub(crate) fn snapshot_program_counters() -> ProgramCounters {
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    lock.lock().map(|guard| guard.clone()).unwrap_or_default()
}
