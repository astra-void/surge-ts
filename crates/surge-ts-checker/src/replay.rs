//! Ordered-delta pipelined replay for the speculative-checking commit tails.
//!
//! The three speculative commit walks (preliminary/final module analysis and
//! the per-file check phase) validate each worker's speculative result in
//! serial file order and, on a conflict, recompute (recheck) the file. That
//! recompute historically ran *serially* on the coordinator — and conflicted
//! files are the expensive ones, so the recheck tail erased most of the
//! parallel worker gain.
//!
//! This module replaces the serial recheck with a **frontier + background
//! replay pool**. The coordinator still publishes in strict serial order (the
//! frontier), so the committed cache state is always exactly the deltas from
//! positions `< frontier`. Meanwhile a pool of replay threads recomputes
//! predicted-conflict files *ahead* of the frontier, reading only the
//! finalized committed store. When the frontier reaches a conflict its replay
//! is usually already computed and valid; if it is stale (a position between
//! the replay's launch and the frontier published a key the replay missed) the
//! coordinator recomputes it inline — the exact old serial recheck, guaranteed
//! valid.
//!
//! ## Why miss-validation alone is sound (no explicit hit-validation)
//!
//! Publication is strictly in serial order, so the committed store never
//! contains a position `>= k` while a replay of `k` runs (the coordinator only
//! writes positions `< frontier <= k`). Therefore:
//!
//! * A "future hit" (observing a value published by `j >= k`) is structurally
//!   impossible — the value is not in the committed store yet.
//! * A committed hit `X = v_p` has `p < frontier <= k`; first-writer-wins makes
//!   `p` the earliest publisher of `X` among positions `< k` (any earlier
//!   publisher is already finalized and would own the slot), and positions in
//!   `[frontier, k)` are all `>= frontier > p`, so they can never become an
//!   earlier publisher. Hence `v_p` equals the serial-visible value at `k`.
//! * The only way a replay diverges from serial is by *missing* a key that a
//!   position `< k` published after the replay read the store. That is caught
//!   by validating the replay's recorded misses against the published-digest
//!   set at frontier time (`misses ∩ published == ∅`), exactly the existing
//!   [`crate::speculative::commit_file_log`] check.
//!
//! So the mission's hit-validation requirement is satisfied *structurally* by
//! in-order publication rather than by a per-hit runtime check: a hit with
//! publisher position `>= k` cannot be observed, and every observable hit is
//! provably the serial-visible value. Replays read only committed state and
//! their own private overlay, so they never carry cross-file overlay
//! dependencies. Analysis additionally validates utility-diagnostic-key
//! additions against the published key set (see the analysis commit walk).
//!
//! The orchestrator here is phase-agnostic: it schedules the pool and drives
//! the frontier, delegating the phase-specific commit/validate/recompute work
//! to closures. The preliminary/final analysis and check tails all share it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

use surge_ts_types::fx::FxHashMap;

/// Configuration for one pipelined commit walk.
#[derive(Clone, Copy)]
pub(crate) struct PipelineConfig {
    /// Number of positions (files/modules) to process, `[0, n)`.
    pub(crate) n: usize,
    /// Background replay threads.
    pub(crate) worker_count: usize,
}

/// What the coordinator did with a position: committed it, or needs a fresh
/// replay (the current one was stale or absent). On `NeedsReplay` the
/// orchestrator re-submits the position to the pool; since the frontier is
/// parked at that position, the re-replay reads the complete `committed<k` and
/// is guaranteed valid — moving the heavy recompute onto a pool thread instead
/// of the coordinator, so it never contends with the pool's look-ahead work.
pub(crate) enum CommitOutcome {
    Committed,
    NeedsReplay,
}

/// Scheduling counters the orchestrator itself tracks (phase-specific
/// commit/validate counts are tracked by the coordinator closure).
#[derive(Debug, Default, Clone)]
pub(crate) struct ReplayStats {
    /// Replay tasks dispatched to the pool.
    pub(crate) submitted: u64,
    /// Positions re-submitted because the pre-computed replay was stale.
    pub(crate) resubmitted: u64,
    /// Replay results the coordinator never consumed (over-prediction).
    pub(crate) wasted: u64,
    /// Max positions submitted-but-not-yet-consumed at any instant (pending
    /// replay depth — a proxy for peak pending-session memory).
    pub(crate) peak_in_flight: u64,
}

/// A replay's delivered outcome: a computed result the coordinator can validate,
/// or a deferral — the replay read a key an earlier not-yet-committed position
/// (`publisher`) will publish, so it ran against an incomplete view and must be
/// re-run once that publisher commits. The deferred attempt's output is
/// discarded (it is one more discarded speculative attempt, exactly like a stale
/// replay), so no result value is carried.
enum Delivered<R> {
    Valid(R),
    Deferred { publisher: usize },
}

/// A minimal FIFO task queue + result map shared between the coordinator and
/// the replay pool. Ascending submission order + FIFO dispatch means the
/// smallest outstanding position (the one the frontier needs next) is computed
/// first.
struct Pool<R> {
    inner: Mutex<PoolInner<R>>,
    task_ready: Condvar,
    result_ready: Condvar,
}

struct PoolInner<R> {
    queue: VecDeque<usize>,
    results: FxHashMap<usize, Delivered<R>>,
    /// `(position, blocking publisher)` for each deferral delivered since the
    /// coordinator last drained — the requeue worklist.
    newly_deferred: Vec<(usize, usize)>,
    shutdown: bool,
}

impl<R> Pool<R> {
    fn new() -> Self {
        Self {
            inner: Mutex::new(PoolInner {
                queue: VecDeque::new(),
                results: FxHashMap::default(),
                newly_deferred: Vec::new(),
                shutdown: false,
            }),
            task_ready: Condvar::new(),
            result_ready: Condvar::new(),
        }
    }

    fn submit(&self, position: usize) {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        inner.queue.push_back(position);
        drop(inner);
        self.task_ready.notify_one();
    }

    /// Blocks until a task is available or the pool is shut down (`None`).
    fn next_task(&self) -> Option<usize> {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        loop {
            if let Some(position) = inner.queue.pop_front() {
                return Some(position);
            }
            if inner.shutdown {
                return None;
            }
            inner = self.task_ready.wait(inner).expect("replay pool poisoned");
        }
    }

    fn deliver(&self, position: usize, delivered: Delivered<R>) {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        if let Delivered::Deferred { publisher } = &delivered {
            inner.newly_deferred.push((position, *publisher));
        }
        inner.results.insert(position, delivered);
        drop(inner);
        self.result_ready.notify_all();
    }

    /// Blocks until a delivery for `position` is available (the latest wins if
    /// the position was re-submitted).
    fn take(&self, position: usize) -> Delivered<R> {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        loop {
            if let Some(result) = inner.results.remove(&position) {
                return result;
            }
            inner = self.result_ready.wait(inner).expect("replay pool poisoned");
        }
    }

    /// Non-blocking: takes the accumulated deferral worklist.
    fn drain_newly_deferred(&self) -> Vec<(usize, usize)> {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        std::mem::take(&mut inner.newly_deferred)
    }

    fn shutdown(&self) {
        let mut inner = self.inner.lock().expect("replay pool poisoned");
        inner.shutdown = true;
        drop(inner);
        self.task_ready.notify_all();
    }

    fn pending_results(&self) -> usize {
        self.inner
            .lock()
            .map(|inner| inner.results.len())
            .unwrap_or(0)
    }
}

/// Drives positions `[0, n)` through an in-order frontier backed by a
/// background replay pool.
///
/// * `shared` — read-only data the replay closure needs (parsed files, live
///   cache handles, …). Shared with pool threads (`Sync`).
/// * `make_thread` — builds one replay thread's reusable mutable state (e.g. a
///   per-thread `CheckerContext` clone). Called once per pool thread.
/// * `replay_one` — recomputes position `k` on a pool thread against the live
///   committed store. Returns `(result, deferred_until)`: `deferred_until =
///   Some(p)` means the replay read a key that an earlier not-yet-committed
///   position `p` will publish, so it ran against an incomplete view and must be
///   re-run once `p` commits — its `result` is discarded (one more discarded
///   speculative attempt, exactly like a stale replay). `None` means it ran to
///   completion and `result` is the outcome the coordinator will validate.
/// * `is_active` — whether a position participates (non-modules are skipped).
/// * `submit_at` — per position, the frontier index at which the pool should
///   start its replay, or `usize::MAX` to never pre-replay it. A conflict may be
///   submitted eagerly (before all its dependencies commit); if it then defers,
///   the orchestrator requeues it when the blocking publisher commits — so the
///   schedule is only a hint and correctness never depends on it (a deferred,
///   stale, or absent replay falls back to the inline recompute in
///   `commit_position` once the frontier reaches the position, where
///   `committed<k` is complete).
/// * `commit_position` — coordinator step: given `k` and an optional pre-computed
///   replay (`None` if not pre-submitted, deferred, or stale), commit the worker
///   result if it validates, else the replay if it validates, else recompute
///   inline.
///
/// Returns the accumulated [`ReplayStats`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_frontier_pipeline<S, T, R>(
    config: PipelineConfig,
    submit_at: &[usize],
    shared: &S,
    make_thread: impl Fn() -> T + Sync,
    replay_one: impl Fn(&S, &mut T, usize) -> (R, Option<usize>) + Sync,
    is_active: impl Fn(usize) -> bool,
    mut commit_position: impl FnMut(usize, Option<R>, bool) -> CommitOutcome,
) -> ReplayStats
where
    S: Sync,
    T: Send,
    R: Send,
{
    let n = config.n;
    let pool = Pool::<R>::new();
    let pool_ref = &pool;
    let mut stats = ReplayStats::default();

    // Bucket positions by the frontier index at which they become submittable.
    let mut submit_buckets: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
    for (position, &at) in submit_at.iter().enumerate().take(n) {
        if at != usize::MAX {
            submit_buckets[at.min(n)].push(position);
        }
    }

    // A replay that deferred too many times stops being requeued and falls to the
    // inline recheck when the frontier reaches it (bounded fallback, no livelock).
    const MAX_DEFERS: u32 = 4;

    std::thread::scope(|scope| {
        for _ in 0..config.worker_count.max(1) {
            let make_thread = &make_thread;
            let replay_one = &replay_one;
            scope.spawn(move || {
                let mut thread_state = make_thread();
                while let Some(position) = pool_ref.next_task() {
                    let (result, deferred_until) = replay_one(shared, &mut thread_state, position);
                    let delivered = match deferred_until {
                        Some(publisher) => Delivered::Deferred { publisher },
                        None => Delivered::Valid(result),
                    };
                    pool_ref.deliver(position, delivered);
                }
            });
        }

        let mut submitted = vec![false; n];
        let mut defer_count: FxHashMap<usize, u32> = FxHashMap::default();
        // Positions waiting on a blocking publisher, keyed by that publisher's
        // position: released (re-submitted) when the frontier commits it.
        let mut parked: FxHashMap<usize, Vec<usize>> = FxHashMap::default();

        for frontier in 0..n {
            // 1. Positions whose scheduled submit index is this frontier.
            for &position in &submit_buckets[frontier] {
                if !submitted[position] {
                    submitted[position] = true;
                    pool_ref.submit(position);
                    stats.submitted += 1;
                }
            }
            // 2. Wake replays parked on the publisher that just committed
            //    (position `frontier - 1` was finalized before this iteration).
            if frontier > 0
                && let Some(list) = parked.remove(&(frontier - 1))
            {
                for position in list {
                    if position > frontier {
                        pool_ref.submit(position);
                        stats.resubmitted += 1;
                    }
                }
            }
            // 3. Route deferrals delivered since the last drain: re-submit if the
            //    blocking publisher already committed, else park until it does.
            for (position, publisher) in pool_ref.drain_newly_deferred() {
                if position <= frontier {
                    continue; // at/behind the frontier — resolved inline
                }
                let count = defer_count.entry(position).or_insert(0);
                *count += 1;
                if *count > MAX_DEFERS {
                    continue; // bounded fallback: the frontier will inline it
                }
                if publisher < frontier {
                    pool_ref.submit(position);
                    stats.resubmitted += 1;
                } else {
                    parked.entry(publisher).or_default().push(position);
                }
            }
            stats.peak_in_flight = stats.peak_in_flight.max(pool_ref.pending_results() as u64);

            if !is_active(frontier) {
                continue;
            }
            // 4. Frontier position: use its latest valid delivery; a deferred or
            //    absent one falls to the inline recheck in `commit_position`,
            //    where `committed<frontier` is complete and always validates.
            let replay = if submitted[frontier] {
                match pool_ref.take(frontier) {
                    Delivered::Valid(result) => Some(result),
                    Delivered::Deferred { .. } => None,
                }
            } else {
                None
            };
            if let CommitOutcome::NeedsReplay = commit_position(frontier, replay, false) {
                // Re-run against the now-complete committed<frontier and finalize.
                pool_ref.submit(frontier);
                stats.resubmitted += 1;
                let replay = match pool_ref.take(frontier) {
                    Delivered::Valid(result) => Some(result),
                    Delivered::Deferred { .. } => None,
                };
                let _ = commit_position(frontier, replay, true);
            }
        }
        pool_ref.shutdown();
        stats.wasted = pool_ref.pending_results() as u64;
    });

    stats
}

/// Monotonic per-run counter for unique replay attempt stamps, so a re-run of a
/// file never reuses a discarded attempt's environment identities. Distinct
/// files also get distinct stamps, which is harmless (environment identity
/// already discriminates by file).
static REPLAY_ATTEMPT: AtomicU64 = AtomicU64::new(2);

/// A fresh attempt stamp `>= 2` (worker speculation is attempt 0, the classic
/// single inline recheck used attempt 1).
pub(crate) fn next_replay_attempt() -> u64 {
    REPLAY_ATTEMPT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use surge_ts_types::fx::FxHashSet;

    // ------------------------------------------------------------------
    // Abstract model of the real six-cache frontier semantics. Keys and values
    // are `u64`; a digest is `key % digest_modulus` (a small modulus forces
    // collisions on demand); the committed store is first-writer-wins. Workers
    // run round-robin with per-worker overlays, exactly like the real dispatch,
    // so overlay-dependency invalidation (a consumer of a conflicted producer's
    // speculative value) is exercised faithfully.
    // ------------------------------------------------------------------

    #[derive(Clone)]
    struct Position {
        reads: Vec<u64>,
    }

    fn value_of(position: usize, key: u64) -> u64 {
        (position as u64)
            .wrapping_mul(1_000_003)
            .wrapping_add(key.wrapping_mul(97))
    }

    struct AbstractProgram {
        positions: Vec<Position>,
        digest_modulus: u64,
    }

    impl AbstractProgram {
        fn digest(&self, key: u64) -> u64 {
            key % self.digest_modulus
        }
    }

    /// First-writer-wins committed store: key -> (publisher position, value).
    #[derive(Default, Clone)]
    struct Cache {
        entries: FxHashMap<u64, (usize, u64)>,
    }

    /// The serial oracle: positions in strict order against one growing cache.
    fn serial_oracle(program: &AbstractProgram) -> (Vec<Vec<u64>>, Cache) {
        let mut cache = Cache::default();
        let mut outputs = Vec::with_capacity(program.positions.len());
        for (position, spec) in program.positions.iter().enumerate() {
            let mut output = Vec::with_capacity(spec.reads.len());
            for &key in &spec.reads {
                if let Some(&(_, value)) = cache.entries.get(&key) {
                    output.push(value);
                } else {
                    let value = value_of(position, key);
                    cache.entries.insert(key, (position, value));
                    output.push(value);
                }
            }
            outputs.push(output);
        }
        (outputs, cache)
    }

    /// One position's observations against a base cache view plus an overlay.
    #[derive(Clone)]
    struct Observed {
        misses: FxHashSet<u64>,
        overlay_deps: FxHashSet<usize>,
        inserts: Vec<(u64, u64)>,
        output: Vec<u64>,
    }

    /// Compute a position against `base` (committed) layered under `overlay`
    /// (a shared per-worker overlay of earlier same-worker inserts, keyed by
    /// producing position). Records misses by digest, overlay-hit dependencies,
    /// and first-seen inserts. `delay` perturbs completion order.
    fn compute(
        program: &AbstractProgram,
        position: usize,
        base: &Cache,
        overlay: &mut FxHashMap<u64, (usize, u64)>,
        delay: bool,
    ) -> Observed {
        if delay {
            let spins = (200u64.saturating_sub(position as u64 % 200)) * 60;
            let mut acc = 0u64;
            for i in 0..spins {
                acc = acc.wrapping_add(i);
            }
            std::hint::black_box(acc);
        }
        let spec = &program.positions[position];
        let mut misses = FxHashSet::default();
        let mut overlay_deps = FxHashSet::default();
        let mut inserts = Vec::new();
        let mut output = Vec::with_capacity(spec.reads.len());
        for &key in &spec.reads {
            if let Some(&(_, value)) = base.entries.get(&key) {
                output.push(value);
            } else if let Some(&(producer, value)) = overlay.get(&key) {
                if producer != position {
                    overlay_deps.insert(producer);
                }
                output.push(value);
            } else {
                misses.insert(program.digest(key));
                let value = value_of(position, key);
                overlay.insert(key, (position, value));
                inserts.push((key, value));
                output.push(value);
            }
        }
        Observed {
            misses,
            overlay_deps,
            inserts,
            output,
        }
    }

    /// A single-file replay: base = committed, fresh empty overlay (no deps).
    fn compute_replay(
        program: &AbstractProgram,
        position: usize,
        base: &Cache,
        delay: bool,
    ) -> Observed {
        let mut overlay = FxHashMap::default();
        let obs = compute(program, position, base, &mut overlay, delay);
        debug_assert!(
            obs.overlay_deps.is_empty(),
            "replay overlay must be private"
        );
        obs
    }

    /// A replay plus the reservation-model deferral decision: a key this position
    /// reads that is not yet committed and whose serial first-writer is an earlier
    /// position defers to that publisher (the latest such, so the requeued run
    /// reads the most-committed view). Mirrors the real reservation query.
    fn compute_replay_deferring(
        program: &AbstractProgram,
        position: usize,
        base: &Cache,
        first_writer: &FxHashMap<u64, usize>,
        delay: bool,
    ) -> (Observed, Option<usize>) {
        let obs = compute_replay(program, position, base, delay);
        let mut deferred_until: Option<usize> = None;
        for &key in &program.positions[position].reads {
            if base.entries.contains_key(&key) {
                continue;
            }
            if let Some(&writer) = first_writer.get(&key)
                && writer < position
            {
                deferred_until = Some(deferred_until.map_or(writer, |c| c.max(writer)));
            }
        }
        (obs, deferred_until)
    }

    struct Shared {
        program: AbstractProgram,
        committed: StdMutex<Cache>,
        first_writer: FxHashMap<u64, usize>,
        delay: bool,
    }

    /// Simulate the worker phase: round-robin dispatch, per-worker overlay,
    /// empty fan-out snapshot. Returns each position's worker log.
    fn worker_pass(program: &AbstractProgram, worker_count: usize) -> Vec<Observed> {
        let n = program.positions.len();
        let mut logs: Vec<Option<Observed>> = (0..n).map(|_| None).collect();
        let workers = worker_count.max(1);
        let mut overlays: Vec<FxHashMap<u64, (usize, u64)>> =
            (0..workers).map(|_| FxHashMap::default()).collect();
        let snapshot = Cache::default();
        for position in 0..n {
            let worker = position % workers; // ascending dispatch within a worker
            let obs = compute(program, position, &snapshot, &mut overlays[worker], false);
            logs[position] = Some(obs);
        }
        logs.into_iter().map(|o| o.unwrap()).collect()
    }

    /// Runs the orchestrator over the abstract program and asserts per-position
    /// outputs and final committed values exactly match the serial oracle.
    /// `submit_mode` selects how the schedule is derived, so both the
    /// dependency-driven schedule and the degenerate submit-everything-at-0
    /// schedule (which maximizes stale replays and inline fallbacks) are
    /// exercised for the same programs.
    fn run_and_assert_impl(
        program: AbstractProgram,
        worker_count: usize,
        submit_mode: SubmitMode,
        delay: bool,
    ) {
        let (oracle_outputs, oracle_cache) = serial_oracle(&program);
        let n = program.positions.len();
        let worker_logs = worker_pass(&program, worker_count);

        // Prediction (superset of true conflicts; scheduling hint only): a
        // position that misses or overlay-depends on an earlier insert.
        let mut earlier: FxHashSet<u64> = FxHashSet::default();
        let mut predicted = vec![false; n];
        for position in 0..n {
            let log = &worker_logs[position];
            if log.misses.iter().any(|d| earlier.contains(d)) || !log.overlay_deps.is_empty() {
                predicted[position] = true;
            }
            for &(key, _) in &log.inserts {
                earlier.insert(program.digest(key));
            }
        }

        // Dependency-driven submit schedule (mirrors
        // `speculative::compute_submit_schedule` on the abstract model).
        let submit_at = match submit_mode {
            SubmitMode::Immediate => (0..n)
                .map(|p| if predicted[p] { 0 } else { usize::MAX })
                .collect::<Vec<_>>(),
            SubmitMode::DependencyDriven => {
                let mut first_writer: FxHashMap<u64, usize> = FxHashMap::default();
                let mut schedule = vec![usize::MAX; n];
                for position in 0..n {
                    let log = &worker_logs[position];
                    if predicted[position] {
                        let mut latest = None;
                        for d in &log.misses {
                            if let Some(&producer) = first_writer.get(d) {
                                if producer < position {
                                    latest =
                                        Some(latest.map_or(producer, |c: usize| c.max(producer)));
                                }
                            }
                        }
                        schedule[position] = latest.map_or(0, |p| p + 1);
                    }
                    for &(key, _) in &log.inserts {
                        first_writer.entry(program.digest(key)).or_insert(position);
                    }
                }
                schedule
            }
        };

        // The serial first-writer of each key = the reservation publisher a
        // replay defers to when it reads that key before the writer commits.
        let first_writer: FxHashMap<u64, usize> = oracle_cache
            .entries
            .iter()
            .map(|(&key, &(position, _))| (key, position))
            .collect();

        let shared = Shared {
            program,
            committed: StdMutex::new(Cache::default()),
            first_writer,
            delay,
        };
        let mut published: FxHashSet<u64> = FxHashSet::default();
        let mut replayed: FxHashSet<usize> = FxHashSet::default();
        let mut outputs: Vec<Option<Vec<u64>>> = vec![None; n];

        let apply = |committed: &StdMutex<Cache>,
                     published: &mut FxHashSet<u64>,
                     position: usize,
                     obs: &Observed| {
            let mut cache = committed.lock().unwrap();
            for &(key, value) in &obs.inserts {
                cache.entries.entry(key).or_insert((position, value));
            }
            for d in &obs.misses {
                published.insert(*d);
            }
        };

        run_frontier_pipeline(
            PipelineConfig { n, worker_count },
            &submit_at,
            &shared,
            || (),
            |shared: &Shared, _: &mut (), position: usize| {
                let view = shared.committed.lock().unwrap().clone();
                compute_replay_deferring(
                    &shared.program,
                    position,
                    &view,
                    &shared.first_writer,
                    shared.delay,
                )
            },
            |_position| true,
            |position, replay, is_final| {
                if !replayed.contains(&position) {
                    let worker = &worker_logs[position];
                    let worker_clean = worker.misses.is_disjoint(&published)
                        && !worker.overlay_deps.iter().any(|f| replayed.contains(f));
                    if worker_clean {
                        apply(&shared.committed, &mut published, position, worker);
                        outputs[position] = Some(worker.output.clone());
                        return CommitOutcome::Committed;
                    }
                    replayed.insert(position);
                }
                if let Some(replay) = replay {
                    if replay.misses.is_disjoint(&published) {
                        apply(&shared.committed, &mut published, position, &replay);
                        outputs[position] = Some(replay.output.clone());
                        return CommitOutcome::Committed;
                    }
                }
                if !is_final {
                    return CommitOutcome::NeedsReplay;
                }
                let view = shared.committed.lock().unwrap().clone();
                let obs = compute_replay(&shared.program, position, &view, false);
                apply(&shared.committed, &mut published, position, &obs);
                outputs[position] = Some(obs.output);
                CommitOutcome::Committed
            },
        );

        let final_cache = shared.committed.into_inner().unwrap();
        for position in 0..n {
            assert_eq!(
                outputs[position].as_ref().unwrap(),
                &oracle_outputs[position],
                "output divergence at position {position} (workers={worker_count})"
            );
        }
        let final_values: FxHashMap<u64, u64> = final_cache
            .entries
            .iter()
            .map(|(&k, &(_, v))| (k, v))
            .collect();
        let oracle_values: FxHashMap<u64, u64> = oracle_cache
            .entries
            .iter()
            .map(|(&k, &(_, v))| (k, v))
            .collect();
        assert_eq!(final_values, oracle_values, "committed cache divergence");
    }

    #[derive(Clone, Copy)]
    enum SubmitMode {
        /// Submit every predicted conflict at frontier 0 — maximizes stale
        /// replays and inline fallbacks, stressing the fallback path.
        Immediate,
        /// Dependency-driven: submit each conflict when its last dependency
        /// commits (the production schedule).
        DependencyDriven,
    }

    fn clone_program(program: &AbstractProgram) -> AbstractProgram {
        AbstractProgram {
            positions: program.positions.clone(),
            digest_modulus: program.digest_modulus,
        }
    }

    fn run_and_assert(program: AbstractProgram, worker_count: usize, _window: usize) {
        // Every program is checked under both schedules.
        run_and_assert_impl(
            clone_program(&program),
            worker_count,
            SubmitMode::Immediate,
            false,
        );
        run_and_assert_impl(program, worker_count, SubmitMode::DependencyDriven, false);
    }

    fn pos(reads: &[u64]) -> Position {
        Position {
            reads: reads.to_vec(),
        }
    }

    // Scenario 1: future hit — a replay of k must never observe j>k's insert.
    #[test]
    fn future_hit_is_never_observed() {
        let mut positions = vec![pos(&[]); 6];
        positions[0] = pos(&[7]);
        positions[5] = pos(&[7]);
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            4,
            8,
        );
    }

    // Scenario 2: miss becomes an earlier hit.
    #[test]
    fn miss_becomes_earlier_hit() {
        let mut positions = vec![pos(&[]); 5];
        positions[2] = pos(&[3]);
        positions[4] = pos(&[3]);
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            4,
            8,
        );
    }

    // Scenario 3: earlier conflicted publisher invalidates a later validated file.
    #[test]
    fn earlier_conflicted_publisher_invalidates_later() {
        let positions = vec![pos(&[]), pos(&[10]), pos(&[10]), pos(&[10])];
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            4,
            8,
        );
    }

    // Scenario 4: transitive chain k -> j -> m.
    #[test]
    fn transitive_invalidation_chain() {
        let positions = vec![pos(&[100]), pos(&[100, 200]), pos(&[200, 300]), pos(&[300])];
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            4,
            8,
        );
    }

    // Scenario 5: same key read by many positions -> all see first publisher.
    #[test]
    fn same_key_repeated_reads() {
        let positions: Vec<Position> = (0..20).map(|_| pos(&[42])).collect();
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            4,
            6,
        );
    }

    // Scenario 6: digest collisions -> conservative (never-wrong) replay.
    #[test]
    fn digest_collisions_stay_correct() {
        let positions: Vec<Position> = (0..40)
            .map(|i| pos(&[(i as u64) * 7 + 1, (i as u64) % 5]))
            .collect();
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 4,
            },
            4,
            10,
        );
    }

    // Scenario 7: consumed speculative overlay — a conflicted producer must
    // invalidate its same-worker consumer. With 2 workers, positions 0 and 2
    // land on worker 0; 2 reads a key 0 inserts (overlay hit -> dep on 0). If 0
    // conflicts, 2 must be invalidated and replayed.
    #[test]
    fn consumed_speculative_overlay_propagates() {
        // Position 1 (worker 1) inserts key 5 into committed via a conflict.
        // Position 0 (worker 0) also inserts key 5 speculatively; position 2
        // (worker 0) reads key 5 -> overlay dep on 0. Serial: 0 inserts 5, 2
        // hits 0. But add an earlier real publisher to force 0 itself to be a
        // conflict-free vs not — keep it simple: 0 inserts 5, 2 depends on 0,
        // and 1 (between them) also reads 5 forcing ordering checks.
        let positions = vec![pos(&[5, 9]), pos(&[5]), pos(&[5, 9])];
        // Force worker assignment 0->w0, 1->w1, 2->w0 by using 2 workers.
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            2,
            8,
        );
    }

    // Scenario 8: dense dependency chain -> inline fallback, no livelock.
    #[test]
    fn dense_chain_falls_back_without_livelock() {
        let mut positions = vec![pos(&[0])];
        for i in 1..60u64 {
            positions.push(pos(&[i - 1, i]));
        }
        run_and_assert(
            AbstractProgram {
                positions,
                digest_modulus: 1_000,
            },
            8,
            32,
        );
    }

    // Scenario 9: injected delays / many worker counts stay deterministic.
    #[test]
    fn injected_delays_stay_deterministic() {
        let make = || {
            (0..80u64)
                .map(|i| pos(&[i % 13, (i * 7) % 30, i]))
                .collect::<Vec<_>>()
        };
        for &workers in &[2usize, 4, 8, 3, 5] {
            for _ in 0..10 {
                run_and_assert_impl(
                    AbstractProgram {
                        positions: make(),
                        digest_modulus: 7,
                    },
                    workers,
                    SubmitMode::Immediate,
                    true,
                );
                run_and_assert_impl(
                    AbstractProgram {
                        positions: make(),
                        digest_modulus: 7,
                    },
                    workers,
                    SubmitMode::DependencyDriven,
                    true,
                );
            }
        }
    }

    // Degenerate sizes must not hang.
    #[test]
    fn empty_and_singleton() {
        run_and_assert(
            AbstractProgram {
                positions: vec![],
                digest_modulus: 10,
            },
            4,
            8,
        );
        run_and_assert(
            AbstractProgram {
                positions: vec![pos(&[1, 2, 3])],
                digest_modulus: 10,
            },
            4,
            8,
        );
    }
}
