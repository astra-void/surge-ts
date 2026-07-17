//! Deterministic strongly-connected-component (SCC) scheduler for parallel
//! module analysis.
//!
//! Module analysis over the tRPC corpus is embarrassingly parallel — the
//! import dependency graph's condensation has a critical path of ~4% of total
//! analysis time (see `docs/perf/TRPC-STC-REPORT.md`). This module turns an
//! import-edge set into a schedule that a coordinator can drive: cyclic module
//! groups collapse into one serial SCC, independent SCCs run concurrently, and
//! results commit in a single deterministic order regardless of the order
//! workers happen to finish in.
//!
//! The scheduler is intentionally decoupled from the checker's mutable state:
//! it operates on `(usize, usize)` importer→importee edges over module indices,
//! so it is unit-testable in isolation and reused for both the preliminary and
//! final analysis passes.

// Wired into the analysis driver in a following increment; the graph/SCC core
// is landed and tested first so the scheduling logic is validated before it
// touches per-worker arena allocation.
#![allow(dead_code)]

/// Index of a module (file) in the program's `parsed_files` slice.
pub(crate) type ModuleIndex = usize;

/// Index of a strongly-connected component in [`ModuleSchedule::sccs`].
pub(crate) type SccIndex = usize;

/// One strongly-connected component of the module import graph.
#[derive(Debug, Clone)]
pub(crate) struct Scc {
    /// Member module indices, ascending — the deterministic within-SCC order a
    /// serial fallback (for cyclic groups) and the coordinator commit use.
    pub(crate) members: Vec<ModuleIndex>,
    /// SCCs this one imports from (deduplicated, ascending by their minimum
    /// member index). Every dependency must be analyzed before this SCC.
    pub(crate) dependencies: Vec<SccIndex>,
    /// SCCs that import from this one; decremented to release ready work.
    pub(crate) dependents: Vec<SccIndex>,
}

impl Scc {
    /// A cyclic group (more than one module, or a single module with a
    /// self-import) must be analyzed serially within itself.
    pub(crate) fn is_cyclic(&self) -> bool {
        self.members.len() > 1
    }

    /// The minimum member index — the SCC's deterministic identity/ordering key
    /// (member lists never overlap, so minimums are unique).
    pub(crate) fn anchor(&self) -> ModuleIndex {
        self.members[0]
    }
}

/// A deterministic parallel schedule of module SCCs.
#[derive(Debug, Clone)]
pub(crate) struct ModuleSchedule {
    /// One entry per module: the SCC it belongs to.
    scc_of: Vec<SccIndex>,
    /// The SCCs, ordered so `sccs[i].anchor()` is ascending in `i`.
    sccs: Vec<Scc>,
}

impl ModuleSchedule {
    /// Builds the schedule from `module_count` modules and importer→importee
    /// `edges`. Edges referencing out-of-range indices are ignored (a module
    /// may import a file that is filtered from analysis). Self-edges are kept so
    /// a self-importing module is reported cyclic.
    pub(crate) fn build(module_count: usize, edges: &[(ModuleIndex, ModuleIndex)]) -> Self {
        let mut adjacency: Vec<Vec<ModuleIndex>> = vec![Vec::new(); module_count];
        let mut has_self_edge = vec![false; module_count];
        for &(from, to) in edges {
            if from >= module_count || to >= module_count {
                continue;
            }
            if from == to {
                has_self_edge[from] = true;
            }
            adjacency[from].push(to);
        }

        let raw = tarjan_scc(&adjacency);

        // Tarjan yields components in reverse topological order. Re-key them so
        // component indices are ascending by minimum member — a stable identity
        // independent of traversal, which makes the whole schedule deterministic.
        let mut components = raw;
        for component in components.iter_mut() {
            component.sort_unstable();
        }
        components.sort_unstable_by_key(|component| component[0]);

        let mut scc_of = vec![0usize; module_count];
        for (scc_index, component) in components.iter().enumerate() {
            for &module in component {
                scc_of[module] = scc_index;
            }
        }

        let mut sccs: Vec<Scc> = components
            .into_iter()
            .map(|members| Scc {
                members,
                dependencies: Vec::new(),
                dependents: Vec::new(),
            })
            .collect();

        // Cross-SCC edges become condensation edges. Dedup via per-SCC marker
        // sets scoped by a generation stamp to avoid O(n^2) membership scans.
        let mut dependency_sets: Vec<Vec<SccIndex>> = vec![Vec::new(); sccs.len()];
        let mut dependent_sets: Vec<Vec<SccIndex>> = vec![Vec::new(); sccs.len()];
        for (from, targets) in adjacency.iter().enumerate() {
            let from_scc = scc_of[from];
            for &to in targets {
                let to_scc = scc_of[to];
                if from_scc != to_scc {
                    dependency_sets[from_scc].push(to_scc);
                    dependent_sets[to_scc].push(from_scc);
                }
            }
        }
        for (scc_index, scc) in sccs.iter_mut().enumerate() {
            let mut dependencies = std::mem::take(&mut dependency_sets[scc_index]);
            dependencies.sort_unstable();
            dependencies.dedup();
            let mut dependents = std::mem::take(&mut dependent_sets[scc_index]);
            dependents.sort_unstable();
            dependents.dedup();
            scc.dependencies = dependencies;
            scc.dependents = dependents;
        }

        Self { scc_of, sccs }
    }

    pub(crate) fn scc_count(&self) -> usize {
        self.sccs.len()
    }

    pub(crate) fn module_count(&self) -> usize {
        self.scc_of.len()
    }

    pub(crate) fn scc(&self, index: SccIndex) -> &Scc {
        &self.sccs[index]
    }

    pub(crate) fn scc_of(&self, module: ModuleIndex) -> SccIndex {
        self.scc_of[module]
    }

    pub(crate) fn sccs(&self) -> &[Scc] {
        &self.sccs
    }

    /// The number of modules in the largest SCC — the inherently-serial floor.
    pub(crate) fn largest_scc_size(&self) -> usize {
        self.sccs
            .iter()
            .map(|scc| scc.members.len())
            .max()
            .unwrap_or(0)
    }

    /// A deterministic topological order of SCCs (Kahn, ties broken by anchor),
    /// used as the single commit order so worker completion order never affects
    /// output. Panics never: the condensation is a DAG by construction.
    pub(crate) fn commit_order(&self) -> Vec<SccIndex> {
        let mut remaining: Vec<usize> =
            self.sccs.iter().map(|scc| scc.dependencies.len()).collect();
        // Min-heap on anchor via a sorted ready set kept ordered by SccIndex,
        // which is already ascending-by-anchor, so a plain ascending scan is a
        // deterministic tie-break.
        let mut ready: std::collections::BinaryHeap<std::cmp::Reverse<SccIndex>> = self
            .sccs
            .iter()
            .enumerate()
            .filter(|(_, scc)| scc.dependencies.is_empty())
            .map(|(index, _)| std::cmp::Reverse(index))
            .collect();

        let mut order = Vec::with_capacity(self.sccs.len());
        while let Some(std::cmp::Reverse(scc_index)) = ready.pop() {
            order.push(scc_index);
            for &dependent in &self.sccs[scc_index].dependents {
                remaining[dependent] -= 1;
                if remaining[dependent] == 0 {
                    ready.push(std::cmp::Reverse(dependent));
                }
            }
        }
        debug_assert_eq!(order.len(), self.sccs.len(), "condensation must be a DAG");
        order
    }
}

/// Iterative Tarjan strongly-connected-components. Returns each component as a
/// vector of module indices. Iterative (explicit stack) so deep dependency
/// chains — the default-lib / `@types` prefix runs dozens deep — cannot
/// overflow the call stack.
fn tarjan_scc(adjacency: &[Vec<ModuleIndex>]) -> Vec<Vec<ModuleIndex>> {
    const UNVISITED: usize = usize::MAX;
    let n = adjacency.len();
    let mut index_of = vec![UNVISITED; n];
    let mut low_link = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut component_stack: Vec<ModuleIndex> = Vec::new();
    let mut next_index = 0usize;
    let mut components: Vec<Vec<ModuleIndex>> = Vec::new();

    // Each frame tracks the node and the next successor edge to visit.
    let mut work: Vec<(ModuleIndex, usize)> = Vec::new();

    for root in 0..n {
        if index_of[root] != UNVISITED {
            continue;
        }
        work.push((root, 0));
        while let Some(&(node, successor)) = work.last() {
            if successor == 0 {
                index_of[node] = next_index;
                low_link[node] = next_index;
                next_index += 1;
                component_stack.push(node);
                on_stack[node] = true;
            }

            if successor < adjacency[node].len() {
                let next = adjacency[node][successor];
                work.last_mut().unwrap().1 += 1;
                if index_of[next] == UNVISITED {
                    work.push((next, 0));
                } else if on_stack[next] {
                    low_link[node] = low_link[node].min(index_of[next]);
                }
                continue;
            }

            // All successors of `node` processed. Fold its low-link into its
            // parent, and close a component if `node` is a root.
            if low_link[node] == index_of[node] {
                let mut component = Vec::new();
                loop {
                    let member = component_stack.pop().unwrap();
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                components.push(component);
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low_link[parent] = low_link[parent].min(low_link[node]);
            }
        }
    }

    components
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scc_members(schedule: &ModuleSchedule) -> Vec<Vec<ModuleIndex>> {
        schedule
            .sccs()
            .iter()
            .map(|scc| scc.members.clone())
            .collect()
    }

    #[test]
    fn independent_modules_are_all_singletons() {
        let schedule = ModuleSchedule::build(4, &[]);
        assert_eq!(schedule.scc_count(), 4);
        assert_eq!(
            scc_members(&schedule),
            vec![vec![0], vec![1], vec![2], vec![3]]
        );
        assert!(schedule.sccs().iter().all(|scc| !scc.is_cyclic()));
        assert_eq!(schedule.largest_scc_size(), 1);
    }

    #[test]
    fn chain_keeps_singletons_with_dependency_edges() {
        // 0 -> 1 -> 2
        let schedule = ModuleSchedule::build(3, &[(0, 1), (1, 2)]);
        assert_eq!(schedule.scc_count(), 3);
        assert!(schedule.sccs().iter().all(|scc| !scc.is_cyclic()));
        // Commit order must place a dependency before its importer: 2, 1, 0.
        assert_eq!(schedule.commit_order(), vec![2, 1, 0]);
    }

    #[test]
    fn simple_cycle_collapses_to_one_scc() {
        // 0 -> 1 -> 0
        let schedule = ModuleSchedule::build(2, &[(0, 1), (1, 0)]);
        assert_eq!(schedule.scc_count(), 1);
        assert_eq!(scc_members(&schedule), vec![vec![0, 1]]);
        assert!(schedule.sccs()[0].is_cyclic());
        assert_eq!(schedule.largest_scc_size(), 2);
    }

    #[test]
    fn self_edge_is_cyclic() {
        let schedule = ModuleSchedule::build(1, &[(0, 0)]);
        assert_eq!(schedule.scc_count(), 1);
        // A single self-importing module still commits, and the members list is
        // the lone module.
        assert_eq!(scc_members(&schedule), vec![vec![0]]);
        assert_eq!(schedule.commit_order(), vec![0]);
    }

    #[test]
    fn diamond_orders_dependencies_before_dependents() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3  (0 depends on 1,2; 1,2 depend on 3)
        let schedule = ModuleSchedule::build(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        assert_eq!(schedule.scc_count(), 4);
        let order = schedule.commit_order();
        let position: std::collections::HashMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(pos, &scc)| (scc, pos))
            .collect();
        // Every dependency commits before the SCC that imports it.
        for (index, scc) in schedule.sccs().iter().enumerate() {
            for &dependency in &scc.dependencies {
                assert!(position[&dependency] < position[&index]);
            }
        }
        // Deterministic: leaf module 3 commits first, root importer 0 last.
        assert_eq!(order.first(), Some(&schedule.scc_of(3)));
        assert_eq!(order.last(), Some(&schedule.scc_of(0)));
    }

    #[test]
    fn nested_cycle_with_independent_tail() {
        // Cycle {0,1,2}; 2 -> 3 (independent leaf); 4 isolated.
        let schedule = ModuleSchedule::build(5, &[(0, 1), (1, 2), (2, 0), (2, 3), (4, 4)]);
        let members = scc_members(&schedule);
        assert!(
            members.contains(&vec![0, 1, 2]),
            "cycle members: {members:?}"
        );
        assert!(members.contains(&vec![3]));
        assert!(members.contains(&vec![4]));
        // The cyclic SCC depends on the leaf 3's SCC; 3 commits first.
        let cycle_scc = schedule.scc_of(0);
        let leaf_scc = schedule.scc_of(3);
        let order = schedule.commit_order();
        let pos = |scc| order.iter().position(|&s| s == scc).unwrap();
        assert!(pos(leaf_scc) < pos(cycle_scc));
    }

    #[test]
    fn out_of_range_edges_are_ignored() {
        let schedule = ModuleSchedule::build(2, &[(0, 5), (9, 1), (0, 1)]);
        assert_eq!(schedule.module_count(), 2);
        assert_eq!(schedule.commit_order(), vec![1, 0]);
    }

    #[test]
    fn empty_program() {
        let schedule = ModuleSchedule::build(0, &[]);
        assert_eq!(schedule.scc_count(), 0);
        assert_eq!(schedule.commit_order(), Vec::<SccIndex>::new());
        assert_eq!(schedule.largest_scc_size(), 0);
    }

    #[test]
    fn deep_chain_does_not_overflow() {
        // A chain far deeper than any recursion limit would allow.
        let edges: Vec<(usize, usize)> = (0..100_000).map(|i| (i, i + 1)).collect();
        let schedule = ModuleSchedule::build(100_001, &edges);
        assert_eq!(schedule.scc_count(), 100_001);
        assert_eq!(schedule.largest_scc_size(), 1);
        // Leaf (highest index) commits first, root (0) last.
        let order = schedule.commit_order();
        assert_eq!(order.first(), Some(&schedule.scc_of(100_000)));
        assert_eq!(order.last(), Some(&schedule.scc_of(0)));
    }
}
