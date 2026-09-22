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

    // The only transition is an unassigned local becoming assigned, and only
    // when every continuing branch assigned it — so it must be in each branch's
    // delta. Walking the smallest delta instead of every visible local keeps a
    // merge proportional to what the branches wrote, not to function size.
    if has_fallthrough_base {
        record_flow_branch_merge_fast_path_count();
        return;
    }
    let driver = continuing_branches
        .iter()
        .min_by_key(|branch| branch.changed_local_count)
        .expect("checked non-empty above");

    let mut updates: Vec<(usize, Arc<str>)> = Vec::new();
    for (&scope_index, scope_changes) in &driver.changes {
        let Some(base_scope) = flow_state.scopes.get(scope_index) else {
            continue;
        };
        for (name, driver_state) in scope_changes {
            record_flow_branch_merge_local_iteration_count(1);
            if *driver_state != AssignmentState::Assigned
                || base_scope.locals.get(name) != Some(&AssignmentState::DeclaredUnassigned)
            {
                continue;
            }
            let assigned_everywhere = continuing_branches.iter().all(|branch| {
                branch.get(scope_index, name) == Some(AssignmentState::Assigned)
            });
            if assigned_everywhere {
                updates.push((scope_index, name.clone()));
            }
        }
    }

    if updates.is_empty() {
        record_flow_branch_merge_fast_path_count();
        return;
    }

    for (scope_index, name) in updates {
        if let Some(scope) = flow_state.scopes.get_mut(scope_index) {
            scope.locals.insert(name, AssignmentState::Assigned);
        }
    }
}
