//! Program-wide performance counters and the global counter store.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::timings::ProgramTimings;

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
    pub(crate) module_type_dedup_hit_count: u64,
    pub(crate) module_type_dedup_insert_count: u64,
    pub(crate) module_type_dedup_nominal_match_count: u64,
    pub(crate) module_type_dedup_structural_comparison_count: u64,
    pub(crate) module_type_dedup_structural_nodes_visited_count: u64,
    pub(crate) module_type_dedup_forced_lazy_peel_count: u64,
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
    pub(crate) canonicalize_call_count: u64,
    pub(crate) canonicalize_cache_hit_count: u64,
    pub(crate) canonicalize_syscall_count: u64,
    pub(crate) canonicalize_syscall_nanos: u64,
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
    pub(crate) generic_type_cache_hit_count: u64,
    pub(crate) generic_type_cache_miss_count: u64,
    pub(crate) generic_type_cache_insert_count: u64,
    pub(crate) generic_type_cache_capped_count: u64,
    pub(crate) instantiation_intern_hit_count: u64,
    pub(crate) instantiation_intern_insert_count: u64,
    pub(crate) instantiation_intern_capped_count: u64,
    pub(crate) signature_context_generic_hit_count: u64,
    pub(crate) signature_context_generic_store_count: u64,
    pub(crate) named_type_cache_hit_count: u64,
    pub(crate) named_type_cache_insert_count: u64,
    pub(crate) lazy_reference_create_count: u64,
    pub(crate) lazy_reference_peel_count: u64,
    pub(crate) lazy_reference_clean_expansion_count: u64,
    pub(crate) lazy_reference_memo_hit_count: u64,
    pub(crate) lazy_reference_interner_hit_count: u64,
    pub(crate) lazy_reference_blocked_count: u64,
    pub(crate) lazy_reference_degraded_expansion_count: u64,
    pub(crate) lazy_reference_degraded_memo_hit_count: u64,
    pub(crate) generic_instantiation_count: u64,
    pub(crate) lazy_intersection_create_count: u64,
    pub(crate) lazy_intersection_peel_count: u64,
    pub(crate) lazy_reference_peel_reason_counts: Vec<u64>,
    pub(crate) lazy_annotation_reference_create_count: u64,
    pub(crate) function_signatures_indexed_count: u64,
    pub(crate) lazy_signature_create_count: u64,
    pub(crate) lazy_signature_parameter_annotation_create_count: u64,
    pub(crate) lazy_signature_return_annotation_create_count: u64,
    pub(crate) lazy_signature_generic_annotation_create_count: u64,
    pub(crate) lazy_signature_materialization_count: u64,
    pub(crate) signature_materialization_cache_hit_count: u64,
    pub(crate) signature_materialization_cache_miss_count: u64,
    pub(crate) clean_signature_expansion_count: u64,
    pub(crate) degraded_signature_expansion_count: u64,
    pub(crate) unique_degraded_signature_expansion_count: u64,
    pub(crate) repeated_degraded_signature_expansion_count: u64,
    pub(crate) max_degraded_signature_expansion_repeats: u64,
    pub(crate) overload_group_create_count: u64,
    pub(crate) signature_structural_clone_count: u64,
    pub(crate) lazy_signature_annotation_handle_size_bytes: u64,
    pub(crate) lazy_signature_environment_handle_size_bytes: u64,
    pub(crate) lazy_signature_parameter_slot_size_bytes: u64,
    pub(crate) lazy_signature_estimated_shallow_retained_bytes: u64,
    pub(crate) lazy_signature_environment_create_count: u64,
    pub(crate) lazy_signature_environment_reference_count: u64,
    pub(crate) interface_resolution_attempt_count: u64,
    pub(crate) interface_resolution_success_count: u64,
    pub(crate) interface_resolution_degraded_count: u64,
    pub(crate) unique_stable_interface_declaration_count: u64,
    pub(crate) unique_interface_instantiation_tuple_count: u64,
    pub(crate) duplicate_clean_interface_instantiation_count: u64,
    pub(crate) duplicate_degraded_interface_instantiation_count: u64,
    pub(crate) interface_own_property_map_alloc_count: u64,
    pub(crate) interface_method_signature_group_alloc_count: u64,
    pub(crate) interface_call_signature_array_alloc_count: u64,
    pub(crate) interface_construct_signature_array_alloc_count: u64,
    pub(crate) interface_index_signature_alloc_count: u64,
    pub(crate) inherited_member_merge_attempt_count: u64,
    pub(crate) inherited_member_merge_cache_hit_count: u64,
    pub(crate) overload_array_alloc_count: u64,
    pub(crate) physical_interface_cache_hit_count: u64,
    pub(crate) physical_interface_cache_miss_count: u64,
    pub(crate) physical_interface_cache_insert_count: u64,
    pub(crate) physical_interface_cache_racing_insert_count: u64,
    pub(crate) physical_interface_cache_skip_disabled_count: u64,
    pub(crate) physical_interface_cache_skip_unstable_declaration_count: u64,
    pub(crate) physical_interface_cache_skip_unresolved_argument_count: u64,
    pub(crate) physical_interface_cache_skip_unsupported_argument_count: u64,
    pub(crate) physical_interface_cache_reject_had_error_count: u64,
    pub(crate) physical_interface_cache_reject_diagnostics_count: u64,
    pub(crate) physical_interface_cache_reject_degradation_count: u64,
    pub(crate) physical_interface_cache_reject_unknown_count: u64,
    pub(crate) physical_interface_cache_reject_context_count: u64,
    pub(crate) physical_interface_cache_reject_traversal_count: u64,
    pub(crate) physical_interface_cache_key_bytes: u64,
    pub(crate) physical_interface_cache_value_shallow_bytes: u64,
    pub(crate) interface_member_declaration_visit_count: u64,
    pub(crate) unique_interface_member_declaration_count: u64,
    pub(crate) interface_method_mapping_attempt_count: u64,
    pub(crate) unique_interface_method_instantiation_count: u64,
    pub(crate) duplicate_clean_interface_method_mapping_count: u64,
    pub(crate) duplicate_degraded_interface_method_mapping_count: u64,
    pub(crate) interface_overload_group_construction_attempt_count: u64,
    pub(crate) unique_interface_overload_declaration_set_count: u64,
    pub(crate) unique_interface_overload_instantiation_count: u64,
    pub(crate) duplicate_interface_overload_construction_count: u64,
    pub(crate) clean_reusable_interface_member_count: u64,
    pub(crate) non_reusable_interface_member_count: u64,
    pub(crate) context_retaining_interface_member_count: u64,
    pub(crate) unknown_containing_interface_member_count: u64,
    pub(crate) interface_template_build_attempt_count: u64,
    pub(crate) interface_template_insert_count: u64,
    pub(crate) interface_template_hit_count: u64,
    pub(crate) interface_template_member_visit_avoided_count: u64,
    pub(crate) interface_template_retained_bytes: u64,
    pub(crate) interface_method_cache_hit_count: u64,
    pub(crate) interface_method_cache_miss_count: u64,
    pub(crate) interface_method_cache_insert_count: u64,
    pub(crate) interface_method_cache_reject_had_error_count: u64,
    pub(crate) interface_method_cache_reject_diagnostics_count: u64,
    pub(crate) interface_method_cache_reject_degradation_count: u64,
    pub(crate) interface_method_cache_reject_unknown_count: u64,
    pub(crate) interface_method_cache_reject_context_count: u64,
    pub(crate) interface_method_cache_reject_contextual_typing_count: u64,
    pub(crate) interface_method_cache_reject_traversal_count: u64,
    pub(crate) interface_method_cache_key_bytes: u64,
    pub(crate) interface_method_cache_value_shallow_bytes: u64,
    pub(crate) interface_method_function_payload_avoided_count: u64,
    pub(crate) interface_overload_cache_hit_count: u64,
    pub(crate) interface_overload_cache_miss_count: u64,
    pub(crate) interface_overload_cache_insert_count: u64,
    pub(crate) interface_overload_cache_reject_count: u64,
    pub(crate) interface_overload_cache_key_bytes: u64,
    pub(crate) interface_overload_cache_value_shallow_bytes: u64,
    pub(crate) interface_overload_array_avoided_count: u64,
    pub(crate) interface_overload_function_payload_avoided_count: u64,
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

pub(crate) fn counters_enabled() -> bool {
    COUNTERS_ENABLED.load(Ordering::Relaxed)
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

pub(crate) fn record_canonicalize_call() {
    record_program_counter(|c| c.canonicalize_call_count += 1);
}

pub(crate) fn record_canonicalize_cache_hit() {
    record_program_counter(|c| c.canonicalize_cache_hit_count += 1);
}

pub(crate) fn record_canonicalize_syscall(elapsed: Duration) {
    record_program_counter(|c| {
        c.canonicalize_syscall_count += 1;
        c.canonicalize_syscall_nanos += elapsed.as_nanos() as u64;
    });
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
