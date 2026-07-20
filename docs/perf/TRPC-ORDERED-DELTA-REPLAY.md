# tRPC ordered-delta pipelined replay — engineering report & blocker

Branch `main`, session commits `a7d9a6b` (primitive + tests) and `8c5822f`
(check-tail integration) on top of `2155be4`. Hardware: Apple M1 Pro (10
cores), 16 GB, macOS 27.0, rustc 1.94.0, mimalloc, release profile. Fixture:
`.local-projects/trpc` @ `3e0e9793eb7f8c4cfbe70a1dccb72f8d355e3c8b`. Canonical
command: `target/release/surge --project .local-projects/trpc/tsconfig.json
--format json --maxDiagnostics 10000 --jobs auto`.

## Executive summary

The mission was to replace the three serial speculative-recheck tails with a
sound, deterministic **ordered-delta pipelined replay** and thereby cut the
combined tail wall time from ~4.55 s (system allocator) / ~3.24 s (mimalloc)
toward ≤ 2.0 s.

**Outcome:** a sound, byte-identical, well-tested ordered-delta replay
mechanism (`crate::replay`) is implemented and integrated into the per-file
check tail. It is **byte-for-byte identical** to serial across tRPC/zod/ky/
ofetch at `--jobs 1/2/4/8` and the opt-in full parallel-analysis path (10×
deterministic on zod), passes `nextest 1565/1565` including 10 adversarial
replay tests, and does not regress the shipping default.

**But it does not reduce wall time on tRPC** (interleaved A/B: pipeline 9.74 s
vs serial recheck 9.80 s at `--jobs 8` — within noise). The tail's conflict
structure fundamentally resists this class of parallelism. This report
documents the mechanism, the measured blocker, every variant tried, and the
next concrete code change. Per the mission's fallback clause it is delivered as
the blocker report, with the validated mechanism kept (non-regressing) rather
than reverted.

The **shipping `--jobs auto` default is unchanged** (10.2 s median, 1.97 GB,
sha `4d69a2d5`): auto runs the serial check path, so the pipeline affects only
explicit `--jobs N ≥ 2`.

## The mechanism (`crate::replay`)

A phase-agnostic **frontier orchestrator** + background replay pool:

* **In-order publication (the frontier).** The coordinator commits positions in
  strict serial order, so the committed cache is always exactly the deltas from
  positions `< frontier`. This makes hit-validation *structural*: the committed
  store never contains a position `≥ k` while replaying `k`, so a "future hit"
  is unobservable, and first-writer-wins makes every observable hit the
  serial-visible value. Only **misses** need validating (`misses ∩ published ==
  ∅`) — the existing `commit_file_log` check. The mission's explicit
  hit-validation requirement is satisfied by this invariant rather than a
  per-hit runtime check; the module header carries the full argument.
* **Dependency-driven scheduling** (`compute_submit_schedule`). Each conflict is
  submitted to the pool the moment its last dependency (the latest first-writer
  of a key it missed, plus its overlay producers) commits, so its replay reads a
  committed view already containing its dependencies.
* **Inline fallback.** A stale or absent replay falls back to the inline
  recheck — the exact old serial behavior, guaranteed valid.
* **Tested.** 10 adversarial tests against an abstract first-writer-wins cache
  with per-worker overlays and a serial oracle: future hit, miss-becomes-hit,
  earlier-conflicted-publisher, transitive chains, same-key repeats, digest
  collisions, consumed-overlay propagation, dense chains (inline fallback, no
  livelock), and 100 randomized/injected-delay scheduling runs — under both a
  submit-everything schedule and the dependency-driven schedule.

## Measured blocker

### 1. The conflicts form deep dependency chains

Conservative (digest-based) conflict-dependency DAG on tRPC (`SURGE_REPLAY_DAG`):

| tail | conflicts | level-1 (independent) | max level (round ceiling) |
|---|---|---|---|
| preliminary analysis | 582 | 246 (42%) | 42 |
| final analysis | 192 | 96 (50%) | 19 |
| check | 215 | 57 (27%) | 36 |

Under 50% of conflicts are independent; the rest sit on chains up to 42 deep.
The critical path of a perfectly-pipelined replay is the chain length, and the
chain is long.

### 2. Over-recursion makes the schedule imprecise (the core issue)

A worker speculates against the **fan-out snapshot** (frozen at fan-out); a
replay reads the **committed store**. When a computation misses a key that a
later position will publish, it does not stop — it *recurses* into that key's
sub-resolution and interns spurious sub-instantiations (e.g. a structural
`Auth[]` where serial would have deferred to nominal `ReadonlyArray<Auth>`).
Consequences:

* A replay whose dependency is **another conflict** reads a stale view, over-
  recurses, and its actual dependency set diverges from the worker log — so the
  worker-log-derived schedule cannot predict when it will be valid. Only
  **clean-dependent** conflicts (every dependency is a clean file, which commits
  early and deterministically) replay reliably. Restricting pre-replay to those
  is what keeps the pipeline non-regressing.
* Over-recursion also makes value-based validation **unsound**: committing a
  worker result whose diagnostics match serial still publishes its spurious
  inserts, which pollute the cache for a later file that resolves one directly.
  Measured: 216/222 check conflicts are *false* (diagnostics already equal
  serial), yet 101 of those "benign" worker commits publish more inserts than
  serial — enough to diverge 4 diagnostics. See the `commit_file_log`
  doc-comment.

### 3. Pool/coordinator CPU contention

With `worker_count` pool threads plus the coordinator recomputing conflict-
dependent files inline, an 8-core machine is oversubscribed; the parallel
replays slow the coordinator's inline work by roughly as much as they save. A
re-submit-on-stale variant (coordinator does no heavy work, re-submits to the
pool) removed the inline work but was net-negative — the re-replays still
compete and the critical chain dominates.

### The false-conflict insight and why it doesn't help

216/222 check "conflicts" produce identical *diagnostics* on recheck — the
STC digest validation is conservative, and post-`c46f0f2` a cache miss
recomputes the *same* content-based value. So the recheck's only real job is
producing serial-correct **cache state**. But that state cannot be produced
without reading the complete `committed<k` (the inline recheck), because any
incomplete read over-recurses. So the false-conflict rate cannot be exploited to
avoid the rechecks.

## Variants tried (all byte-identical; all sound)

| variant | result |
|---|---|
| window-based prefetch | net-negative — wasted stale over-recursion |
| dependency-driven, all conflicts | net-negative — 92 stale, peak-in-flight 59, contention |
| **level-1-only + inline fallback** | **neutral** (shipped) — A/B 9.74 vs 9.80 s |
| re-submit-on-stale (no coordinator heavy work) | net-negative + unresolved stale retries |
| value-based commit validation | **unsound** — cache pollution from over-recursion |

## Validation (at `8c5822f`)

* `cargo fmt --check`, `cargo check --workspace`: clean.
* `cargo nextest run --workspace`: **1565/1565** (incl. 10 replay tests).
* `pnpm oracle:test`: green. Full sweep `--all --maxDiagnostics 200`: **97/97,
  0 gating mismatches** (5 message-drift-only, non-gating; unchanged).
* tRPC raw-byte identity: `--jobs 1/2/4/8` + opt-in parallel-analysis all
  identical (sha `4d69a2d5`, 2,190 diagnostics). zod/ky/ofetch: same. 10× zod
  parallel deterministic.
* Memory: opt-in parallel `--jobs 8` peak fp 2.55–2.71 GB (baseline
  2.55–2.68 GB; within the +100 MB gate). Default `--jobs auto` 1.97 GB
  (unchanged). Peak pending-replay depth (`peak_in_flight`) ≤ ~22 with the
  level-1 schedule.

## Final distributions

`--jobs auto` (shipping default; serial check, no pipeline), 5 runs:
10.81 / 10.79 / 10.35 / 10.17 / 10.20 s → **median 10.35 s**, peak fp 1.97 GB,
sha `4d69a2d5`. Unchanged from `2155be4`.

`--jobs 8` opt-in parallel-analysis, interleaved A/B (pipeline vs
`SURGE_REPLAY_OFF=1` serial recheck): 9.74 vs 9.80 s median — no measurable
difference.

## The next concrete code change (blocker)

The critical path today is governed by the *digest* dependency DAG (chains up to
42). The *value* dependency is far shallower — only ~6 conflicts actually change
their output. To let value-shallow dependence govern the critical path, a replay
must be able to resolve any position against a per-position view **without
over-recursing** on a not-yet-committed dependency. That requires a cache
representation change:

1. **Publisher-stamped, versioned cache entries.** Each `program_instantiations`
   / `physical_interface_*` entry carries its first-writer position. A replay at
   `k` resolves, for each key, the earliest entry with position `< k`.
2. **A "resolution-in-progress / deferred" sentinel** interned under a key
   before its owning conflict commits, so a replay that reaches a
   not-yet-committed dependency reads the sentinel and *defers* (nominal form)
   instead of recursing and over-interning. The sentinel is replaced by the real
   value when the dependency commits; a replay that consumed a sentinel records
   the dependency and is re-validated against the committed value.

With (1)+(2), a replay never over-recurses, its dependency set equals its real
value-dependencies, and the scheduler can pipeline against the shallow value DAG
— at which point the ≤ 2.0 s tail target becomes reachable. This is the
versioned-delta cache the mission sketched; it is a genuine representation change
to the six order-visible caches and their `crate::speculative` session, scoped
as its own mission.

Until then, the serial recheck remains the fastest correct tail for tRPC, and
the pipeline is retained as a validated, non-regressing mechanism that benefits
workloads whose conflicts are more independent.

## Follow-up: the representation change was attempted (blocker)

The "next concrete code change" above (publisher-stamped versioned entries + a
resolution-deferred sentinel) was implemented and measured. The reservation
table, deferral measurement, and a byte-safe file-level requeue landed (opt-in
`SURGE_DEFER`), moving inline rechecks 153 → 64 byte-identically — but wall time
did not improve, and the sub-file short-circuit needed to make it a win requires
a mid-resolution nominal return that panicked and was reverted. See
[TRPC-DEFERRED-RESOLUTION.md](TRPC-DEFERRED-RESOLUTION.md).
