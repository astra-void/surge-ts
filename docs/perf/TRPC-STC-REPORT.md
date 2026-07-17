# Speculative Transactional Checking (STC) — engineering report

Branch `perf/speculative-transactional-checking`, started from the validated
performance HEAD `1600ed5` (`a2c7247` code + docs) on
`perf/trpc-allocation-volume`.

Goal: make `--jobs auto` perform real parallel semantic checking that is
byte-identical to `--jobs 1`, driving fresh-process tRPC wall time from
~14.6 s toward 5 s, without changing a single diagnostic byte or exceeding the
memory milestone (peak internal fp ≤ 2.0 GB, finish ≤ 0.70 GB).

Status: **Stage 0 complete** (baseline, phase breakdown, concurrency/mutation
census, divergence characterization, feasibility verdict). Later stages gated
on the scope decision recorded at the end of this section.

---

## Stage 0 — baseline, census, and feasibility

### 0.1 Baseline (STC binary, tRPC `tsconfig.json`, this session/window)

Canonical command:
`surge --project .local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000 --jobs 1`
→ 2,190 diagnostics, sha256 `4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59`.

Internal phase timeline (`SURGE_TIMINGS=1`, cumulative `t=` from RSS stages,
jobs=1):

| phase | wall (Δ) | share |
|---|---|---|
| parsing | 0.28 s | 2% |
| ambient + global collection | 0.70 s | 5% |
| **preliminary module analysis** | **4.28 s** | **32%** |
| **final module analysis** | **3.26 s** | **24%** |
| module_binding | 0.16 s | 1% |
| module_local_values | 1.28 s | 10% |
| **check phase (per-file body checking)** | **2.99 s** | **22%** |
| finish (teardown) | 0.45 s | 3% |
| **total (internal)** | **~13.4 s** | |

Memory (jobs=1): peak fp **1.98 GB**, finish fp **454 MB** (both within gate).

### 0.2 What is already parallel

The mission's premise ("behaves effectively single-threaded") is only partly
true. The codebase already has:

- **Parallel parsing** (`resolve_parse_worker_count`, `thread::scope`).
- **Parallel frontend specifier scan / source read** (`specifier_scan.rs`,
  `lib.rs` read-worker pool).
- **A working parallel check phase** — `check_program_files_parallel`
  (`program/mod.rs:1607`): it freezes every worker-reachable arena
  (`freeze_worker_reachable_arenas`), spawns `worker_count` threads each with a
  cloned `CheckerContext` and its own `program_type_store`, pulls file indices
  off an `AtomicUsize`, and **sorts results by `file_index`** so diagnostic
  *ordering* is already deterministic.

`--jobs N` (N ≥ 2) routes to this parallel check path today. `--jobs auto` is
**deliberately pinned to serial** (`resolve_worker_count`: `AUTO_JOBS ⇒ 1`)
because that path is not byte-identical (see 0.4).

Module analysis — the 7.6 s critical path — is **serial regardless of
`--jobs`**. Only parsing and the 3 s check phase scale with worker count.

### 0.3 Measured worker-count sweep (same window)

| jobs | wall | diags | sha256 | vs serial |
|---|---|---|---|---|
| 1 | 16.98 s¹ | 2190 | `4d69a2d5` | — (canonical) |
| auto | 14.70 s | 2190 | `4d69a2d5` | **identical** (auto = serial) |
| 2 | 12.49 s | 2190 | `da456972` | **diverges** (7 msgs) |
| 4 | 11.39 s | 2190 | `7538df7e` | **diverges** (10 msgs) |
| 8 | 11.07 s | 2190 | `7538df7e` | diverges (same as j4) |

¹ first run, thermally loaded; the comparable in-window serial number is
auto=14.70 s.

**Speedup ceiling of check-phase parallelism ≈ 11 s** (jobs=8). That is exactly
the ~3 s check phase parallelized away, floor-bounded by the untouched ~10 s of
serial analysis + collection + binding + finish.

### 0.4 Divergence characterization (the determinism blocker)

Real parallel checking (jobs ≥ 2) diverges from serial in a **remarkably clean**
way — aligned index-by-index against serial:

- **jobs=2: 7 messages differ. jobs=4/8: 10 messages differ.** All are `TS2322`.
- **Zero code / file / span / ordering differences** in all 2,190 diagnostics.
- Every diff is the identical pattern: the `Auth` array type renders as its
  **nominal** form `ReadonlyArray<Auth> | undefined` (serial) vs its
  **structural** form `Auth[] | undefined` (parallel).
- The divergent set is **worker-count-dependent** (jobs=2 ≠ jobs=4), confirming
  a race rather than a fixed transform.

Mechanism (root-caused): the six program-wide instantiation/resolution caches on
`CheckerContext` —
`program_resolved_generic_types`, `program_instantiations`,
`physical_interface_instantiations`,
`physical_interface_declaration_templates`,
`physical_interface_method_instantiations`,
`physical_interface_overload_instantiations` —
are all `Arc<Mutex<FxHashMap<…>>>`, and the context `Clone` bumps the `Arc`
(`context.rs:297`). So **all check-phase workers share one set of caches.**
Whichever worker first interns the `Auth` array instantiation fixes its display
form (nominal vs structural) for every later reader. Serial always seeds it in
file order (nominal wins); parallel races (structural sometimes wins first).

Upper bound: the `SURGE_CHECK_CACHE_ISOLATION=1` probe (restore all six caches
after every file) flips **28** messages, all `TS2339` — but those 28 do **not**
flip under real parallel execution, because each worker still accumulates
cross-file cache within its own file batch. Real parallel exposure is the 7–10
`TS2322` display sites only.

### 0.5 Concurrency / mutation census (check phase)

Order-sensitive shared mutable state touched during the check phase:

| state | kind | sharing | order-sensitive? | published to output? |
|---|---|---|---|---|
| 6 instantiation/resolution caches (0.4) | `Arc<Mutex<HashMap>>` | shared across workers | **yes** — first-writer-wins on display form | **yes** (10 TS2322) |
| worker-reachable arenas | bump arenas | shared, **frozen** before fan-out | no (read-only) | no |
| `program_type_store` (canonical weak stores) | per-worker clone + `with_program_type_store` | worker-local during phase | no observed divergence | pointer identity feeds assignability fast paths |
| `diagnostics` / `stats` | per-worker `local_ctx` | worker-local, merged + sorted by file_index | no (deterministic merge) | yes (order already deterministic) |
| per-file flow / scope / inference | `begin_file_check` reset | worker-local | no | no |

Arenas are already made safe (frozen → late allocation panics loudly). The
canonical type store is per-worker and did not produce divergence in this
corpus, but its pointer identity is load-bearing for assignability fast paths
and must not be published in a completion-order-dependent way. The **only**
demonstrated output-affecting nondeterminism is the six shared caches.

### 0.6 Feasibility verdict

1. **The 5.5 s final gate is NOT reachable by parallelizing the check phase.**
   The check phase is 3 s of 13.4 s; its parallel ceiling is ~11 s (measured).
   Reaching 5 s **requires parallelizing module analysis** (preliminary 4.28 s +
   final 3.26 s = ~7.6 s, 56% of wall) — the mission's Stage 5, and by far the
   most mutation-intensive, dependency-ordered phase.

2. **Byte-identical parallel checking is not a one-line reroute.** Because the
   six caches are already `Arc<Mutex>`-shared, `jobs=auto → parallel` diverges
   by a first-writer-wins race on one display form. Making it byte-identical
   requires deterministic commit ordering of cache publication (the mission's
   Stage 2-4 transaction/coordinator model) so the "first writer" is always the
   serial-first file — **or** eliminating the cross-file display dependency at
   its source. Normalizing the canonical interner-hit display form is a **known
   safety boundary** (added 10 diagnostics in the allocation program) and is off
   the table.

3. **Achievable-and-safe within the parallelism mandate:** deterministic
   parallel checking (Stages 1-4) → ~11 s byte-identical (meets the Stage 1
   gate ≤ 13.5 s, not the 5 s final gate). The 5 s target is a **Stage 5**
   (parallel module analysis) outcome, which is a large, high-regression-risk
   re-architecture of the phase that builds the shared declaration/export/binding
   tables everything else reads.

### 0.7 Recommended path

- **Stage 1-4 (deterministic parallel check, ~11 s byte-identical):** tractable,
  self-contained, real ~25% win. The determinism fix is deterministic
  publication of the six caches (workers compute against a read-only view +
  local scratch; a coordinator commits cache entries in file order). Worth
  landing regardless of whether Stage 5 follows.
- **Stage 5 (parallel module analysis, the 5 s lever):** the only path to the
  headline goal. High risk, multi-stage; must preserve the arena-freeze model,
  the ordered first-wins declaration completion, and canonical pointer identity.
  Should be scoped and green-lit explicitly before starting.

Serial `--jobs 1` remains the trusted reference path throughout; the ≤3%
serial-regression gate constrains all shared-state refactors.
