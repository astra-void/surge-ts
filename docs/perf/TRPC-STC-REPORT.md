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

---

## Stage 5.0 — module graph, SCC, and Amdahl ceiling (measured)

Two opt-in dumps were added (zero-cost when unset; canonical hash `4d69a2d5`
unchanged with them off):

- `SURGE_MODULE_GRAPH_DUMP=<path>` (import_graph.rs) — every resolved relative /
  `paths` import edge, **including back-edges to already-discovered files**, so
  the full cyclic graph is reconstructable. tRPC: **12,126 edges, 4,455 nodes**
  (1,114 app / 3,341 library). (Bare-package `.d.ts` edges are not followed by
  this scan; they point outward to acyclic library leaves and do not create
  app-source cycles, so the app SCC structure is complete.)
- `SURGE_MODULE_TIME_DUMP=<path>` (binding.rs) — real per-module analysis time,
  both passes. tRPC: **5.39 s total** (prelim 2.72 s + final 2.67 s; the ~2.2 s
  gap to the 7.6 s stage wall is the binding fixpoint + scope construction, not
  per-module work). Split app 2.45 s / library 2.94 s; heaviest single module
  186 ms; cost spread broadly, not concentrated.

Offline Tarjan SCC + condensation on the real weights:

| metric | value |
|---|---|
| total SCCs | 3,317 |
| largest SCC | 281 nodes — **100% library** (`@types`-style cycles), 0 app |
| largest app-containing SCC | 24 app nodes; only 65/1,114 app files in any cycle |
| condensation critical path (real time weights) | **0.23 s (4% of 5.39 s)** |
| **structural parallel ceiling** | **~23×** |
| heaviest SCC by cost | a **singleton**, 0.19 s (3%) |

**Verdict: module analysis is embarrassingly parallel.** The dependency graph is
*not* the bottleneck — the app source is 94% acyclic, the heavy modules are
independent singletons, and the longest dependent chain carries only 0.23 s of
analysis work. The real limits are (a) worker count (≈8–10 cores → ~7× on the
5.39 s per-module work) and (b) the ~2.2 s binding fixpoint, which is
cross-module and needs its own treatment or becomes the new floor.

Note this **overturns** a first-pass estimate from the partial collect-time
proxy (which put ~50% of cost in one 21-node cyclic SCC — `utilsProxy.ts` et
al.). Real per-module timing shows those files are not analysis-cost-dominant;
the proxy measured only type-declaration collection (0.8 s of the 5.39 s).

**Revised feasibility:** 5 s is **structurally reachable**. Rough model with 8
workers: per-module analysis 5.39 s → ~0.8 s; binding fixpoint ~2.2 s (partial
parallelism TBD); check phase ~3 s → ~0.5 s; frontend ~1 s; finish ~0.5 s →
plausibly ~5–6 s, with the binding fixpoint the swing factor. This justifies the
per-worker-arena Stage 5 investment.

---

## Implementation session — what landed, what blocked, verdicts

Commits `b83721a` (check-phase STC), `ef64078` (analysis machinery, gated),
plus the auto-policy commit. All stages validated: nextest 1555/1555,
oracle:test green, canonical hashes intact, whitespace clean.

### Landed: serial-equivalent speculative checking (the historic blocker, solved)

`crate::speculative` makes every parallel check path **byte-identical to
`--jobs 1`** — previously `--jobs 2/4/8` each produced different bytes.
Mechanism: workers never write the six order-visible caches; each speculates
against an immutable fan-out snapshot + a private overlay (installed
per-thread so env-materialized shadow contexts route through it, recognized by
live-handle pointer identity), recording per file its observed misses and
consumed worker-overlay entries. A single-threaded commit publishes insertions
in serial file order; a file whose miss-set intersects earlier publications
(or that consumed an invalidated overlay entry) is re-checked against the
committed state. Induction over file order ⇒ byte-equality. Conflict digests
are equality-consistent (`type_conflict_digest`); collisions cause only
spurious sound rechecks.

Measured (tRPC, 10 workers): 95.7% of files commit clean; ~215 rechecks;
worker phase 3.0 s → ~1.0 s but the ordered serial recheck tail (~2.0 s —
conflicted files are the expensive ones, ~16× average cost) holds the check
phase at parity. **Byte-identity: proven** — jobs 1/2/4/6/8/auto raw-`cmp`
identical on tRPC, zod, ky, ofetch; 5× deterministic.

### Landed (gated off): parallel module analysis machinery

The per-module analysis body is extracted (`analyze_module`) and shared
verbatim between the serial loop and a parallel driver: fresh per-module
worker contexts, speculative sessions, suppression-free ordered diagnostic
merge (`push_collected`), coordinator-side declaration-type dedup (its
representative choice feeds pinned pointer-identity machinery), and arena
ownership transfer at the join (`adopt_current_thread_as_owner` — the debug
owner assert located the exact violation: the serial binding fixpoint inserts
into worker-built export-table arenas). The named-resolution memo was made
module-scoped (byte-inert serially on all corpora), mirroring
`begin_file_check`.

**Proof of the framework:** force-recheck mode (every module re-analyzed on
the coordinator's rolling context through the full transaction pipeline)
reproduces serial bytes exactly on tRPC — coordinator, commit, merge, arenas
all correct.

### The precisely-characterized blocker: environment identity

With real speculation, tRPC gains 2 extra `TS2304` diagnostics (zod is fully
clean). Localized by regime-bisection to 5 library `.d.ts` modules whose
**physical-interface cache keys differ across context instances**:
`DeclarationEnvironmentKey` embeds context pointers
(`resolved_named_types`/scope addresses, generations), so a fresh worker
context derives different environment identities than the serial rolling
context, flipping hit/miss on cache entries whose values are
context-sensitive — invisible to conflict validation (different keys never
collide). The induction covers cache *observations*; value equality
additionally requires **content-based environment identity**. That redesign
(replacing pointer-derived `DeclarationEnvironmentKey` components with
content-derived ones, without disturbing canonicalization discriminators) is
the single lever that unblocks parallel analysis — and would likely also
remove the analysis-phase conflict class entirely.

### Verdicts against the mission gates

| gate | result |
|---|---|
| jobs=1 byte-stability + no regression | ✅ canonical `4d69a2d5`, wall unchanged |
| `--jobs N` byte-identity (any N) | ✅ **new capability** (was divergent) |
| jobs=auto ≤ 10 s (Milestone A) | ❌ not reached — auto held serial (see below) |
| memory: peak ≤ 2.10 / finish ≤ 0.60 GB | ✅ default config (1.96/0.45); parallel check 2.23 GB peak — over gate |
| workspace/oracle/corpora validation | ✅ 1555/1555, oracle green, 4 corpora × jobs=1≡auto raw-cmp |

`--jobs auto` stays serial **on measured grounds, not correctness**: parallel
checking at wall parity + ~6 s extra CPU + 0.27 GB extra peak is net-negative
for defaults. `SURGE_PARALLEL_CHECK_AUTO=1` / `SURGE_PARALLEL_ANALYSIS=1` opt
in for measurement.

### Remaining path to 5 s (ordered by leverage)

1. **Content-based declaration-environment identity** — unblocks parallel
   analysis (5.39 s → ~0.8 s at 8 workers) and shrinks conflict classes.
2. **Pipelined rechecks** (ordered-delta speculation for the conflict tail) —
   turns check-phase parity into ~1.4 s (≈ −1.7 s wall).
3. **Per-file release in the parallel loop** + memory-aware worker cap —
   closes the 2.23 vs 2.10 GB gap.
4. **Binding fixpoint** (~2.2 s) — dirty-module tracking once analysis is
   parallel.
