//! Program phase timings and their rendering (`--timings`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use surge_ts_types::{
    snapshot_function_type_counters, snapshot_function_type_payload_alloc_by_expansion_reason,
    snapshot_union_type_counters,
};

use super::counters::snapshot_program_counters;
use super::rss::{
    current_footprint_bytes, current_rss_bytes, peak_footprint_bytes, peak_rss_bytes,
};

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
    pub(crate) cache_stats: Option<ProgramCacheStats>,
}

/// End-of-run sizes of the program-wide type caches, sampled just before the
/// teardown in `clear_program_type_caches` so the peak retained entry counts
/// are visible alongside the RSS stages.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProgramCacheStats {
    pub(crate) generic_type_buckets: u64,
    pub(crate) generic_type_entries: u64,
    pub(crate) instantiation_buckets: u64,
    pub(crate) instantiation_entries: u64,
    pub(crate) physical_interface_entries: u64,
}

/// One RSS reading taken at a pipeline stage boundary. `current_bytes` is the
/// resident set right after the stage completed; `peak_bytes` is the process
/// high-water mark at the same moment, so a spike inside the stage shows up
/// even when it is released before the boundary. The footprint pair mirrors
/// them using macOS `phys_footprint`, which keeps counting compressed/swapped
/// pages that RSS drops under memory pressure.
#[derive(Debug, Clone)]
pub(crate) struct RssStageSample {
    pub(crate) label: &'static str,
    pub(crate) current_bytes: Option<u64>,
    pub(crate) peak_bytes: Option<u64>,
    pub(crate) footprint_bytes: Option<u64>,
    pub(crate) peak_footprint_bytes: Option<u64>,
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
        footprint_bytes: current_footprint_bytes(),
        peak_footprint_bytes: peak_footprint_bytes(),
        elapsed,
    };
    if let Ok(mut guard) = timings.lock() {
        guard.rss_stages.push(sample);
    }
    pause_if_requested(label);
}

/// Opt-in heap-profiling hook: `SURGE_PAUSE_AT_STAGE=<label>` stops the process
/// with SIGSTOP at that stage boundary so external tools (`malloc_history`,
/// `heap`, `footprint`) can attribute the live heap at a deterministic point.
/// Resume with `kill -CONT`. Diagnostics-only; never active without the env var.
fn pause_if_requested(label: &str) {
    static PAUSE_AT: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let Some(requested) = PAUSE_AT.get_or_init(|| std::env::var("SURGE_PAUSE_AT_STAGE").ok())
    else {
        return;
    };
    if requested != label {
        return;
    }
    let pid = std::process::id();
    eprintln!(
        "SURGE_PAUSE_AT_STAGE: pausing at '{label}' (pid {pid}); resume with kill -CONT {pid}"
    );
    #[cfg(target_os = "macos")]
    unsafe {
        unsafe extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        kill(pid as i32, 17); // SIGSTOP on macOS
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
        "    module_type_dedup_hit_count: {}",
        counters.module_type_dedup_hit_count
    );
    eprintln!(
        "    module_type_dedup_insert_count: {}",
        counters.module_type_dedup_insert_count
    );
    eprintln!(
        "    module_type_dedup_nominal_match_count: {}",
        counters.module_type_dedup_nominal_match_count
    );
    eprintln!(
        "    module_type_dedup_structural_comparison_count: {}",
        counters.module_type_dedup_structural_comparison_count
    );
    eprintln!(
        "    module_type_dedup_structural_nodes_visited_count: {}",
        counters.module_type_dedup_structural_nodes_visited_count
    );
    eprintln!(
        "    module_type_dedup_forced_lazy_peel_count: {}",
        counters.module_type_dedup_forced_lazy_peel_count
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
        "    generic_type_cache_hit_count: {}",
        counters.generic_type_cache_hit_count
    );
    eprintln!(
        "    generic_type_cache_miss_count: {}",
        counters.generic_type_cache_miss_count
    );
    eprintln!(
        "    generic_type_cache_insert_count: {}",
        counters.generic_type_cache_insert_count
    );
    eprintln!(
        "    generic_type_cache_capped_count: {}",
        counters.generic_type_cache_capped_count
    );
    eprintln!(
        "    instantiation_intern_hit_count: {}",
        counters.instantiation_intern_hit_count
    );
    eprintln!(
        "    instantiation_intern_insert_count: {}",
        counters.instantiation_intern_insert_count
    );
    eprintln!(
        "    instantiation_intern_capped_count: {}",
        counters.instantiation_intern_capped_count
    );
    eprintln!(
        "    named_type_cache_hit_count: {}",
        counters.named_type_cache_hit_count
    );
    eprintln!(
        "    named_type_cache_insert_count: {}",
        counters.named_type_cache_insert_count
    );
    eprintln!(
        "    lazy_reference_create_count: {}",
        counters.lazy_reference_create_count
    );
    eprintln!(
        "    lazy_reference_peel_count: {}",
        counters.lazy_reference_peel_count
    );
    eprintln!(
        "    lazy_reference_clean_expansion_count: {}",
        counters.lazy_reference_clean_expansion_count
    );
    eprintln!(
        "    lazy_reference_memo_hit_count: {}",
        counters.lazy_reference_memo_hit_count
    );
    eprintln!(
        "    lazy_reference_interner_hit_count: {}",
        counters.lazy_reference_interner_hit_count
    );
    eprintln!(
        "    lazy_reference_blocked_count: {}",
        counters.lazy_reference_blocked_count
    );
    eprintln!(
        "    lazy_reference_degraded_expansion_count: {}",
        counters.lazy_reference_degraded_expansion_count
    );
    eprintln!(
        "    generic_instantiation_count: {}",
        counters.generic_instantiation_count
    );
    for (name, count) in [
        (
            "interface_resolution_attempt_count",
            counters.interface_resolution_attempt_count,
        ),
        (
            "interface_resolution_success_count",
            counters.interface_resolution_success_count,
        ),
        (
            "interface_resolution_degraded_count",
            counters.interface_resolution_degraded_count,
        ),
        (
            "unique_stable_interface_declaration_count",
            counters.unique_stable_interface_declaration_count,
        ),
        (
            "unique_interface_instantiation_tuple_count",
            counters.unique_interface_instantiation_tuple_count,
        ),
        (
            "duplicate_clean_interface_instantiation_count",
            counters.duplicate_clean_interface_instantiation_count,
        ),
        (
            "duplicate_degraded_interface_instantiation_count",
            counters.duplicate_degraded_interface_instantiation_count,
        ),
        (
            "interface_own_property_map_alloc_count",
            counters.interface_own_property_map_alloc_count,
        ),
        (
            "interface_method_signature_group_alloc_count",
            counters.interface_method_signature_group_alloc_count,
        ),
        (
            "interface_call_signature_array_alloc_count",
            counters.interface_call_signature_array_alloc_count,
        ),
        (
            "interface_construct_signature_array_alloc_count",
            counters.interface_construct_signature_array_alloc_count,
        ),
        (
            "interface_index_signature_alloc_count",
            counters.interface_index_signature_alloc_count,
        ),
        (
            "inherited_member_merge_attempt_count",
            counters.inherited_member_merge_attempt_count,
        ),
        (
            "inherited_member_merge_cache_hit_count",
            counters.inherited_member_merge_cache_hit_count,
        ),
        (
            "overload_array_alloc_count",
            counters.overload_array_alloc_count,
        ),
        (
            "physical_interface_cache_hit_count",
            counters.physical_interface_cache_hit_count,
        ),
        (
            "physical_interface_cache_miss_count",
            counters.physical_interface_cache_miss_count,
        ),
        (
            "physical_interface_cache_insert_count",
            counters.physical_interface_cache_insert_count,
        ),
        (
            "physical_interface_cache_racing_insert_count",
            counters.physical_interface_cache_racing_insert_count,
        ),
        (
            "physical_interface_cache_skip_disabled_count",
            counters.physical_interface_cache_skip_disabled_count,
        ),
        (
            "physical_interface_cache_skip_unstable_declaration_count",
            counters.physical_interface_cache_skip_unstable_declaration_count,
        ),
        (
            "physical_interface_cache_skip_unresolved_argument_count",
            counters.physical_interface_cache_skip_unresolved_argument_count,
        ),
        (
            "physical_interface_cache_skip_unsupported_argument_count",
            counters.physical_interface_cache_skip_unsupported_argument_count,
        ),
        (
            "physical_interface_cache_reject_had_error_count",
            counters.physical_interface_cache_reject_had_error_count,
        ),
        (
            "physical_interface_cache_reject_diagnostics_count",
            counters.physical_interface_cache_reject_diagnostics_count,
        ),
        (
            "physical_interface_cache_reject_degradation_count",
            counters.physical_interface_cache_reject_degradation_count,
        ),
        (
            "physical_interface_cache_reject_unknown_count",
            counters.physical_interface_cache_reject_unknown_count,
        ),
        (
            "physical_interface_cache_reject_context_count",
            counters.physical_interface_cache_reject_context_count,
        ),
        (
            "physical_interface_cache_reject_traversal_count",
            counters.physical_interface_cache_reject_traversal_count,
        ),
        (
            "physical_interface_cache_key_bytes",
            counters.physical_interface_cache_key_bytes,
        ),
        (
            "physical_interface_cache_value_shallow_bytes",
            counters.physical_interface_cache_value_shallow_bytes,
        ),
        (
            "interface_member_declaration_visit_count",
            counters.interface_member_declaration_visit_count,
        ),
        (
            "unique_interface_member_declaration_count",
            counters.unique_interface_member_declaration_count,
        ),
        (
            "interface_method_mapping_attempt_count",
            counters.interface_method_mapping_attempt_count,
        ),
        (
            "unique_interface_method_instantiation_count",
            counters.unique_interface_method_instantiation_count,
        ),
        (
            "duplicate_clean_interface_method_mapping_count",
            counters.duplicate_clean_interface_method_mapping_count,
        ),
        (
            "duplicate_degraded_interface_method_mapping_count",
            counters.duplicate_degraded_interface_method_mapping_count,
        ),
        (
            "interface_overload_group_construction_attempt_count",
            counters.interface_overload_group_construction_attempt_count,
        ),
        (
            "unique_interface_overload_declaration_set_count",
            counters.unique_interface_overload_declaration_set_count,
        ),
        (
            "unique_interface_overload_instantiation_count",
            counters.unique_interface_overload_instantiation_count,
        ),
        (
            "duplicate_interface_overload_construction_count",
            counters.duplicate_interface_overload_construction_count,
        ),
        (
            "clean_reusable_interface_member_count",
            counters.clean_reusable_interface_member_count,
        ),
        (
            "non_reusable_interface_member_count",
            counters.non_reusable_interface_member_count,
        ),
        (
            "context_retaining_interface_member_count",
            counters.context_retaining_interface_member_count,
        ),
        (
            "unknown_containing_interface_member_count",
            counters.unknown_containing_interface_member_count,
        ),
        (
            "interface_template_build_attempt_count",
            counters.interface_template_build_attempt_count,
        ),
        (
            "interface_template_insert_count",
            counters.interface_template_insert_count,
        ),
        (
            "interface_template_hit_count",
            counters.interface_template_hit_count,
        ),
        (
            "interface_template_member_visit_avoided_count",
            counters.interface_template_member_visit_avoided_count,
        ),
        (
            "interface_template_retained_bytes",
            counters.interface_template_retained_bytes,
        ),
        (
            "interface_method_cache_hit_count",
            counters.interface_method_cache_hit_count,
        ),
        (
            "interface_method_cache_miss_count",
            counters.interface_method_cache_miss_count,
        ),
        (
            "interface_method_cache_insert_count",
            counters.interface_method_cache_insert_count,
        ),
        (
            "interface_method_cache_reject_had_error_count",
            counters.interface_method_cache_reject_had_error_count,
        ),
        (
            "interface_method_cache_reject_diagnostics_count",
            counters.interface_method_cache_reject_diagnostics_count,
        ),
        (
            "interface_method_cache_reject_degradation_count",
            counters.interface_method_cache_reject_degradation_count,
        ),
        (
            "interface_method_cache_reject_unknown_count",
            counters.interface_method_cache_reject_unknown_count,
        ),
        (
            "interface_method_cache_reject_context_count",
            counters.interface_method_cache_reject_context_count,
        ),
        (
            "interface_method_cache_reject_contextual_typing_count",
            counters.interface_method_cache_reject_contextual_typing_count,
        ),
        (
            "interface_method_cache_reject_traversal_count",
            counters.interface_method_cache_reject_traversal_count,
        ),
        (
            "interface_method_cache_key_bytes",
            counters.interface_method_cache_key_bytes,
        ),
        (
            "interface_method_cache_value_shallow_bytes",
            counters.interface_method_cache_value_shallow_bytes,
        ),
        (
            "interface_method_function_payload_avoided_count",
            counters.interface_method_function_payload_avoided_count,
        ),
        (
            "interface_overload_cache_hit_count",
            counters.interface_overload_cache_hit_count,
        ),
        (
            "interface_overload_cache_miss_count",
            counters.interface_overload_cache_miss_count,
        ),
        (
            "interface_overload_cache_insert_count",
            counters.interface_overload_cache_insert_count,
        ),
        (
            "interface_overload_cache_reject_count",
            counters.interface_overload_cache_reject_count,
        ),
        (
            "interface_overload_cache_key_bytes",
            counters.interface_overload_cache_key_bytes,
        ),
        (
            "interface_overload_cache_value_shallow_bytes",
            counters.interface_overload_cache_value_shallow_bytes,
        ),
        (
            "interface_overload_array_avoided_count",
            counters.interface_overload_array_avoided_count,
        ),
        (
            "interface_overload_function_payload_avoided_count",
            counters.interface_overload_function_payload_avoided_count,
        ),
    ] {
        eprintln!("    {name}: {count}");
    }
    eprintln!(
        "    lazy_intersection_create_count: {}",
        counters.lazy_intersection_create_count
    );
    eprintln!(
        "    lazy_intersection_peel_count: {}",
        counters.lazy_intersection_peel_count
    );
    eprintln!(
        "    lazy_annotation_reference_create_count: {}",
        counters.lazy_annotation_reference_create_count
    );
    eprintln!(
        "    function_signatures_indexed_count: {}",
        counters.function_signatures_indexed_count
    );
    eprintln!(
        "    lazy_signature_create_count: {}",
        counters.lazy_signature_create_count
    );
    eprintln!(
        "    lazy_signature_parameter_annotation_create_count: {}",
        counters.lazy_signature_parameter_annotation_create_count
    );
    eprintln!(
        "    lazy_signature_return_annotation_create_count: {}",
        counters.lazy_signature_return_annotation_create_count
    );
    eprintln!(
        "    lazy_signature_generic_annotation_create_count: {}",
        counters.lazy_signature_generic_annotation_create_count
    );
    eprintln!(
        "    lazy_signature_materialization_count: {}",
        counters.lazy_signature_materialization_count
    );
    eprintln!(
        "    signature_materialization_cache_hit_count: {}",
        counters.signature_materialization_cache_hit_count
    );
    eprintln!(
        "    signature_materialization_cache_miss_count: {}",
        counters.signature_materialization_cache_miss_count
    );
    eprintln!(
        "    clean_signature_expansion_count: {}",
        counters.clean_signature_expansion_count
    );
    eprintln!(
        "    degraded_signature_expansion_count: {}",
        counters.degraded_signature_expansion_count
    );
    eprintln!(
        "    unique_degraded_signature_expansion_count: {}",
        counters.unique_degraded_signature_expansion_count
    );
    eprintln!(
        "    repeated_degraded_signature_expansion_count: {}",
        counters.repeated_degraded_signature_expansion_count
    );
    eprintln!(
        "    max_degraded_signature_expansion_repeats: {}",
        counters.max_degraded_signature_expansion_repeats
    );
    eprintln!(
        "    overload_group_create_count: {}",
        counters.overload_group_create_count
    );
    eprintln!(
        "    signature_structural_clone_count: {}",
        counters.signature_structural_clone_count
    );
    eprintln!(
        "    lazy_signature_annotation_handle_size_bytes: {}",
        counters.lazy_signature_annotation_handle_size_bytes
    );
    eprintln!(
        "    lazy_signature_environment_handle_size_bytes: {}",
        counters.lazy_signature_environment_handle_size_bytes
    );
    eprintln!(
        "    lazy_signature_parameter_slot_size_bytes: {}",
        counters.lazy_signature_parameter_slot_size_bytes
    );
    eprintln!(
        "    lazy_signature_estimated_shallow_retained_bytes: {}",
        counters.lazy_signature_estimated_shallow_retained_bytes
    );
    eprintln!(
        "    lazy_signature_environment_create_count: {}",
        counters.lazy_signature_environment_create_count
    );
    eprintln!(
        "    lazy_signature_environment_reference_count: {}",
        counters.lazy_signature_environment_reference_count
    );
    const PEEL_REASONS: [&str; 42] = [
        "signature_parameter",
        "signature_return",
        "signature_this_parameter",
        "signature_type_predicate",
        "generic_constraint",
        "generic_default",
        "call_signature",
        "construct_signature",
        "interface_method_mapping",
        "class_method",
        "class_constructor",
        "function_type_annotation",
        "module_export_collection",
        "overload_resolution",
        "call_resolution",
        "construct_resolution",
        "assignability",
        "contextual_typing",
        "generic_inference",
        "property_lookup",
        "indexed_access",
        "conditional_type",
        "mapped_type",
        "intersection_merge",
        "union_normalization",
        "apparent_type",
        "diagnostic_display",
        "module_dedup",
        "interface_resolution",
        "interface_own_property_mapping",
        "interface_call_signature_mapping",
        "interface_construct_signature_mapping",
        "interface_index_signature_mapping",
        "interface_heritage_resolution",
        "inherited_property_merge",
        "inherited_method_merge",
        "overload_array_merge",
        "default_lib_interface_instantiation",
        "dependency_interface_instantiation",
        "generic_substitution",
        "parsed_type_mapping",
        "other",
    ];
    for (reason, count) in PEEL_REASONS
        .iter()
        .zip(counters.lazy_reference_peel_reason_counts)
    {
        eprintln!("    lazy_reference_peel_reason_{reason}_count: {count}");
    }
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
    const FUNCTION_PAYLOAD_REASONS: [&str; 13] = [
        "other",
        "expression_inference",
        "call_resolution",
        "property_call_resolution",
        "function_body_setup",
        "return_checking",
        "expected_type",
        "symbol_table",
        "module_export",
        "scope_or_context",
        "substitution_unchanged",
        "substitution_changed",
        "diagnostic_formatting",
    ];
    for (reason, count) in FUNCTION_PAYLOAD_REASONS
        .iter()
        .zip(function_type_counters.function_type_payload_alloc_by_reason)
    {
        eprintln!("    function_type_payload_alloc_{reason}_count: {count}");
    }
    let function_allocs_by_expansion_reason =
        snapshot_function_type_payload_alloc_by_expansion_reason();
    for reason in super::DtsExpansionReason::ALL {
        eprintln!(
            "    dts_function_type_payload_alloc_{}_count: {}",
            reason.label(),
            function_allocs_by_expansion_reason[reason as usize]
        );
    }
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
            "  {:<label_width$}  rss={:>10}  delta={:>10}  peak={:>10}  fp={:>10}  fp_peak={:>10}  t={:>9}",
            sample.label,
            format_bytes_opt(sample.current_bytes),
            delta,
            format_bytes_opt(sample.peak_bytes),
            format_bytes_opt(sample.footprint_bytes),
            format_bytes_opt(sample.peak_footprint_bytes),
            format_duration(sample.elapsed),
        );
        if sample.current_bytes.is_some() {
            previous_current = sample.current_bytes;
        }
    }
    if let Some(stats) = &timings.cache_stats {
        eprintln!(
            "  cache_stats: generic_type_buckets={} generic_type_entries={} \
             instantiation_buckets={} instantiation_entries={} physical_interface_entries={}",
            stats.generic_type_buckets,
            stats.generic_type_entries,
            stats.instantiation_buckets,
            stats.instantiation_entries,
            stats.physical_interface_entries,
        );
    }
    if super::rss_json_enabled() {
        for sample in &timings.rss_stages {
            eprintln!(
                "{{\"rssStage\":\"{}\",\"rssBytes\":{},\"peakRssBytes\":{},\
                 \"footprintBytes\":{},\"peakFootprintBytes\":{},\"elapsedMs\":{:.3}}}",
                sample.label,
                super::json_u64_opt(sample.current_bytes),
                super::json_u64_opt(sample.peak_bytes),
                super::json_u64_opt(sample.footprint_bytes),
                super::json_u64_opt(sample.peak_footprint_bytes),
                sample.elapsed.as_secs_f64() * 1000.0,
            );
        }
        if let Some(stats) = &timings.cache_stats {
            eprintln!(
                "{{\"rssCacheStats\":{{\"genericTypeBuckets\":{},\"genericTypeEntries\":{},\
                 \"instantiationBuckets\":{},\"instantiationEntries\":{},\
                 \"physicalInterfaceEntries\":{}}}}}",
                stats.generic_type_buckets,
                stats.generic_type_entries,
                stats.instantiation_buckets,
                stats.instantiation_entries,
                stats.physical_interface_entries,
            );
        }
    }
}

pub(crate) fn format_bytes_opt(bytes: Option<u64>) -> String {
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
