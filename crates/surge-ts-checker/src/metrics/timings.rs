//! Program phase timings and their rendering (`--timings`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use surge_ts_types::{snapshot_function_type_counters, snapshot_union_type_counters};

use super::counters::snapshot_program_counters;
use super::rss::{current_rss_bytes, peak_rss_bytes};

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
    pub(crate) rss_stages: Vec<RssStageSample>,
}

/// One RSS reading taken at a pipeline stage boundary. `current_bytes` is the
/// resident set right after the stage completed; `peak_bytes` is the process
/// high-water mark at the same moment, so a spike inside the stage shows up
/// even when it is released before the boundary.
#[derive(Debug, Clone)]
pub(crate) struct RssStageSample {
    pub(crate) label: &'static str,
    pub(crate) current_bytes: Option<u64>,
    pub(crate) peak_bytes: Option<u64>,
    pub(crate) elapsed: Duration,
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

pub(crate) fn record_rss_stage(
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    label: &'static str,
    elapsed: Duration,
) {
    let Some(timings) = timings else {
        return;
    };

    let sample = RssStageSample {
        label,
        current_bytes: current_rss_bytes(),
        peak_bytes: peak_rss_bytes(),
        elapsed,
    };
    if let Ok(mut guard) = timings.lock() {
        guard.rss_stages.push(sample);
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

    let io_counters = snapshot_program_counters();
    eprintln!("  io:");
    eprintln!(
        "    canonicalize_calls: {}",
        io_counters.canonicalize_call_count
    );
    eprintln!(
        "    canonicalize_cache_hits: {}",
        io_counters.canonicalize_cache_hit_count
    );
    eprintln!(
        "    canonicalize_syscalls: {}",
        io_counters.canonicalize_syscall_count
    );
    let canonicalize_hit_rate = if io_counters.canonicalize_call_count == 0 {
        0.0
    } else {
        io_counters.canonicalize_cache_hit_count as f64 / io_counters.canonicalize_call_count as f64
            * 100.0
    };
    eprintln!("    canonicalize_cache_hit_rate: {canonicalize_hit_rate:.1}%");
    eprintln!(
        "    canonicalize_syscall_time: {}",
        format_duration(Duration::from_nanos(io_counters.canonicalize_syscall_nanos))
    );
    let canonicalize_avg_syscall = if io_counters.canonicalize_syscall_count == 0 {
        Duration::ZERO
    } else {
        Duration::from_nanos(
            io_counters.canonicalize_syscall_nanos / io_counters.canonicalize_syscall_count,
        )
    };
    eprintln!(
        "    canonicalize_avg_syscall_time: {}",
        format_duration(canonicalize_avg_syscall)
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

/// Rendered BEFORE the `Timings:` block: the measurement harness parses stderr
/// line-by-line and attributes any `key: value` line after a section header to
/// that section, so this block must not appear inside `Timings:`/`counters:`.
pub(crate) fn render_program_rss_stages(timings: &Arc<Mutex<ProgramTimings>>) {
    let Ok(timings) = timings.lock() else {
        return;
    };
    if timings.rss_stages.is_empty() {
        return;
    }

    eprintln!("RSS stages:");
    let label_width = timings
        .rss_stages
        .iter()
        .map(|sample| sample.label.len())
        .max()
        .unwrap_or(0);
    let mut previous_current: Option<u64> = None;
    for sample in &timings.rss_stages {
        let delta = match (previous_current, sample.current_bytes) {
            (Some(previous), Some(current)) => {
                let signed = current as i64 - previous as i64;
                format!(
                    "{}{}",
                    if signed < 0 { "-" } else { "+" },
                    format_bytes(signed.unsigned_abs())
                )
            }
            _ => "n/a".to_string(),
        };
        eprintln!(
            "  {:<label_width$}  rss={:>10}  delta={:>10}  peak={:>10}  t={:>9}",
            sample.label,
            format_bytes_opt(sample.current_bytes),
            delta,
            format_bytes_opt(sample.peak_bytes),
            format_duration(sample.elapsed),
        );
        if sample.current_bytes.is_some() {
            previous_current = sample.current_bytes;
        }
    }
}

fn format_bytes_opt(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "n/a".to_string(), format_bytes)
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    let mib = bytes as f64 / MIB;
    if mib >= 1024.0 {
        format!("{:.2}GB", mib / 1024.0)
    } else {
        format!("{mib:.1}MB")
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}
