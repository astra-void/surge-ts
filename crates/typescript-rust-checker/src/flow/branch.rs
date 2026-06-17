//! Branch delta merging for conditional flow narrowing.

use super::*;

use std::sync::Arc;

use crate::program::{
    record_flow_branch_merge_count, record_flow_branch_merge_fast_path_count,
    record_flow_branch_merge_local_iteration_count,
};

pub(crate) fn merge_branch_deltas(
    flow_state: &mut FunctionFlowState,
    branches: &[FlowBranchDelta],
    has_fallthrough_base: bool,
) {
    record_flow_branch_merge_count(flow_state.scopes.len());

    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return;
    }

    let continuing_branches: Vec<&FlowBranchDelta> =
        branches.iter().filter(|branch| branch.continues).collect();

    if continuing_branches.is_empty() || continuing_branches.iter().all(|branch| branch.is_empty())
    {
        record_flow_branch_merge_fast_path_count();
        return;
    }

    let mut updates: Vec<(usize, Vec<(Arc<str>, AssignmentState)>)> = Vec::new();
    let mut total_updates = 0usize;

    for scope_index in 0..flow_state.scopes.len() {
        let Some(base_scope) = flow_state.scopes.get(scope_index) else {
            continue;
        };

        let mut scope_updates = Vec::new();

        for (name, base_state) in &base_scope.locals {
            record_flow_branch_merge_local_iteration_count(1);

            let mut assigned_branches = 0usize;
            for branch in &continuing_branches {
                let branch_state = branch.get(scope_index, name).unwrap_or(*base_state);
                if matches!(branch_state, AssignmentState::Assigned) {
                    assigned_branches += 1;
                }
            }

            let merged_state = match base_state {
                AssignmentState::Assigned => AssignmentState::Assigned,
                AssignmentState::DeclaredUnassigned => {
                    if has_fallthrough_base {
                        AssignmentState::DeclaredUnassigned
                    } else if continuing_branches.len() >= 1
                        && assigned_branches == continuing_branches.len()
                    {
                        AssignmentState::Assigned
                    } else {
                        AssignmentState::DeclaredUnassigned
                    }
                }
            };

            if merged_state != *base_state {
                total_updates += 1;
                scope_updates.push((name.clone(), merged_state));
            }
        }

        if !scope_updates.is_empty() {
            updates.push((scope_index, scope_updates));
        }
    }

    if total_updates == 0 {
        record_flow_branch_merge_fast_path_count();
        return;
    }

    for (scope_index, scope_updates) in updates {
        if let Some(scope) = flow_state.scopes.get_mut(scope_index) {
            for (name, merged_state) in scope_updates {
                scope.locals.insert(name, merged_state);
            }
        }
    }
}
