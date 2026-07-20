# tRPC deferred-resolution — engineering report & blocker

Branch `perf/stc-deferred-resolution`, on top of `1a245fd` (the ordered-delta
replay report). Hardware: Apple M1 Pro (10 cores), 16 GB, macOS 27.0,
rustc 1.94.0, mimalloc, release profile. Fixture: `.local-projects/trpc`.
Canonical command: `target/release/surge --project
.local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000
--jobs 8`.

## Executive summary

The [ordered-delta replay blocker](TRPC-ORDERED-DELTA-REPLAY.md) identified the
*over-recursion* representation problem: a replay that reads the committed store
and misses a key an earlier not-yet-committed position will publish does not
stop — it recurses into the declaration body and interns a spurious structural
sub-instantiation, enlarging the conflict-dependency DAG and manufacturing false
conflicts. The proposed fix was **publisher-stamped pending reservations** plus a
**resolution-deferred** result so a replay defers (and is requeued) instead of
over-recursing.

**Outcome:** the full mechanism is built, byte-safe, and measured — including
the mid-resolution short-circuit, now implemented as a **safe tri-state probe**
that no longer panics. It **still does not reduce tRPC wall time.** Honoring
deferral is byte-identical and moves the structural conflict metric (inline
rechecks 153 → 64), but neither the requeue alone (check tail 1.45 → 1.66 s) nor
the requeue plus short-circuit (≈1.88 s, within noise of the 1.81 s serial
recheck) beats the serial recheck: the ceiling is pool/coordinator contention
plus stale-replay churn (~80 stale replays fall back to inline), not the
over-recursion representation, which is now safely handled. All of it is kept,
opt-in behind `SURGE_DEFER`, the shipping default fully unchanged.

An earlier short-circuit variant *did* panic intermittently (empty output,
aborted run). Root cause: `Type::peeled` is `reference.resolve().peeled()`, so a
deferred nominal that re-deferred when re-forced made peeling recurse forever →
stack overflow. Fixed by deferring each key **at most once per attempt**
(`WorkerOverlay::deferred_once`): a second lookup of the same key resolves as a
normal miss and terminates the peel. The attempt is discarded and requeued
regardless, so this only bounds work, never changes the committed result.

The **shipping `--jobs auto` default is unchanged** (SHA
`4d69a2d5…5ee59`, 2,190 diagnostics): auto runs the serial check path and the
whole deferral mechanism is behind an opt-in env flag.

## What landed (byte-safe, opt-in)

1. **Reservation primitive** (`crate::speculative::ReservationTable`). A table of
   publisher-stamped `Pending`/`Ready`/`Cancelled` claims kept alongside the six
   order-visible caches. Equality is by exact key — and exact argument vectors
   for the bucketed generic/instantiation caches — never by the 64-bit conflict
   digest, so a digest collision cannot merge distinct keys. Positional
   visibility: the earliest `Pending` publisher `< k` wins; `Ready` (already
   committed) and `Cancelled` (owner did not publish) never defer; a publisher
   `>= k` (a future position, or the querying position itself) is invisible, so a
   replay never waits on itself or observes a future publication. 13 unit tests
   cover all 12 specified reservation properties.

2. **Deferral measurement** (`SURGE_DEFER_MEASURE`, now folded into `SURGE_DEFER`).
   Seeds the table from the worker logs (each position reserves the keys it
   publishes, stamped by serial position) and counts, at the six lookup miss
   sites, how often a replay defers to an earlier pending publisher.

3. **Requeue substrate** (`SURGE_DEFER`). `Delivered::{Valid,Deferred}`;
   `replay_one` returns `(result, deferred_until)`; the frontier orchestrator
   parks a deferred attempt and re-submits it once the blocking publisher
   commits, bounded by `MAX_DEFERS = 4` before the inline recheck at the frontier
   guarantees progress (no livelock). A deferred attempt's diagnostics and
   private cache overlay are discarded exactly like a stale replay's, so honoring
   deferral stays in the pipeline's already-proven byte-identical regime. The 10
   replay oracle tests are extended to model deferral against serial
   first-writers and pass under both the eager and dependency-driven schedules.

4. **Safe short-circuit** (`InstantiationProbe::{Hit,Miss,Deferred}`). On a
   deferred interner miss the lazy peel returns the nominal reference (via
   `make_recursive_cycle_reference`) instead of expanding, with the defer-once
   guard that keeps `Type::peeled` terminating. Byte-identical on all four
   corpora; no thread-local, no panic.

## Measured results (tRPC, `--jobs 8`)

### Deferral fires, and the requeue moves the structural metric

| metric | baseline (clean-only schedule) | `SURGE_DEFER=1` |
|---|---|---|
| pool-committed replays | 47 | **75** |
| inline-recheck conflicts | 153 | **64** |
| stale replays | 18 | 80 |
| would-defer lookups | — (3039 measured) | 3049 |
| pending-reservation leak | — | 0 |
| peak pending reservations | — | 10 210 |
| **check-tail commit_phase** | **1.45 s** | **1.66 s** |
| diagnostics / SHA | 2190 / `4d69a2d5` | 2190 / `4d69a2d5` |

Deferral is real (≈3 000 lookups defer), and honoring it converts 89
conflict-dependent inline rechecks into pool-committed replays (inline 153 → 64)
**byte-identically** — the mission's structural target direction. But the check
tail is **not faster**: 1.45 → 1.66 s.

### Why it is not faster

Without a short-circuit, a deferred replay still **runs to completion**
(over-recursing) before it is discarded and requeued, and it may re-run several
times (defer → requeue → defer …). The requeue converts inline work into pool
work but adds re-run churn, and — as the ordered-delta blocker already measured —
the pool threads plus the coordinator oversubscribe the 8 cores, so the parallel
replays do not beat the serial recheck. The requeue is byte-safe and precise, but
it reschedules the same over-recursion rather than eliminating it.

### The short-circuit (now safe) and why it still does not win

To make a deferred replay *cheap*, the lazy instantiation peel must, on a
deferred interner miss, return the **nominal reference un-memoized** instead of
expanding the declaration body — genuine mid-resolution control flow out of
`ResolveReference::resolve_arc(&self) -> Arc<Type>`, a **total** trait method
with no error channel. The safe implementation is a private tri-state probe
(`InstantiationProbe::{Hit,Miss,Deferred}`) at the peel entry: `Deferred` →
nominal reference (via `make_recursive_cycle_reference`), `Miss` → expand, with
the defer-once guard that keeps `Type::peeled` terminating. No thread-local, no
panic, no poisoned lock; nextest green and byte-identical on all four corpora.

But it still does not beat the serial recheck. Two reasons, both measured:

* **Defer-once limits the savings.** A deferred nominal that is later *forced*
  (peeled) expands the key once anyway (over-recursing once, to terminate the
  peel). Only never-forced nominals save their expansion, so much of the
  over-recursion still happens.
* **Contention + stale churn dominate.** ~80 replays still come back stale (their
  real committed-view misses diverge from the worker-log reservations) and fall
  to the inline recheck, and the pool threads plus the coordinator oversubscribe
  the 8 cores — the same ceiling the [ordered-delta report](TRPC-ORDERED-DELTA-REPLAY.md)
  measured. commit_phase: defer ≈1.88 s vs serial ≈1.81 s (interleaved).

### Latest-dependency scheduling was tried — it is worse

The natural "cut stale replays" lever — schedule each conflict at its latest
worker-log dependency (`compute_submit_schedule(clean_only=false)`) instead of
eagerly at frontier 0 — was implemented and measured. It is **byte-identical**
and drives the structural metric hard: **inline rechecks 64 → ~1**,
**pool-committed replays 75 → ~126** (nearly every conflict now commits via the
pool). But it is **slower**, not faster: interleaved same-window A/B,
**commit_phase defer ≈2.31 s vs serial ≈1.71 s (+35 %)**, and stale replays rose
(80 → ~91). Reverted.

Why routing everything to the pool loses: the coordinator blocks at each frontier
position waiting for that position's replay (`take`), and with ~90 stale replays
it repeatedly waits on pool round-trips (behind a queue of mostly-wasted
look-ahead work) instead of computing inline. The lost time is coordination +
stale churn, not an inherently serial chain — the critical-path measurement below
shows the tail *is* parallelizable; the stale replays are what prevent exploiting
it.

### The critical-path ceiling — the tail *is* parallelizable

`SURGE_CRITPATH=1` times each conflict's serial inline recompute and computes the
longest *weighted* dependency chain (the bound even a perfect topological,
unbounded-parallel commit could not beat):

```
conflicts≈220  serial_sum≈1.7s  critical_path≈0.46s  critical_chain_len=6–8
speedup_ceiling ≈ 3.65–4.2x
```

This overturns the earlier "fundamentally serial" reading. The DAG's *unweighted*
depth is ~35, but the *weighted* critical path is only **6–8 conflicts / ~0.46 s**
— the deep chains are made of *cheap* conflicts, and the expensive conflicts are
largely independent. So an ideal parallel commit could take the tail from ~1.7 s
to ~0.46 s (~1.2 s, ~3.65x). The tail is not inherently serial.

### Out-of-order commit was tried — worse, and it isolates the real blocker

Moving stale/absent recomputes off the coordinator onto the pool (return
`NeedsReplay` → re-submit, so pool threads overlap the recomputes) is
**byte-identical** and drives **inline rechecks to 0** — but it is ~2x *slower*:
interleaved A/B **commit_phase ≈3.0 s vs serial ≈1.7 s**. The coordinator blocks
on each re-submitted position's pool round-trip behind a queue of (mostly wasted)
look-ahead work; the round-trip + queue + contention cost dwarfs an inline
recompute. Reverted.

## The blocker, precisely

The ceiling (~0.46 s) is real, but reaching it requires the coordinator to only
ever *validate precomputed-ready* results, never block on a heavy recompute. The
one thing standing in the way is the **~84 stale replays**: a pre-replay that
read the committed store before some earlier position it depends on had
committed. Each stale replay forces the coordinator to either recompute inline
(serial, ~1.7 s total) or block on a pool re-run (~3.0 s) — both lose. Stale
replays exist because reservations are seeded from the **worker logs**, whose
miss/insert sets differ from a replay's *real* committed-view sets, so the
deferral does not fire on every key an earlier position will publish.

### The one lever that matters: eliminate stale replays

Complete reservations ⇒ every pre-replay reads a complete-enough view ⇒ 0 stale ⇒
the coordinator's walk is cheap validate+publish and the pool hits the ~0.46 s
critical path. Concretely:

* **Reserve on the replay's *real* miss/insert set, not the worker log** — e.g. a
  first cheap pass that records each conflict's committed-view dependencies, then
  a topological wave commit. This is the substantive next change.
* A terminal short-circuit nominal (no defer-once re-expansion) is a secondary
  micro-lever; it cannot eliminate stale on its own.

Until stale replays are eliminated, the serial recheck remains the
fastest correct tail for tRPC. The reservation table, the measurement, the
requeue substrate, and the safe short-circuit are kept as the validated,
non-regressing foundation (opt-in `SURGE_DEFER`; the shipping default is
untouched).

## Validation

* `cargo fmt --check`, `cargo check --workspace`: clean.
* `cargo nextest run --workspace`: green (incl. 13 reservation tests + 10 replay
  oracle tests extended for deferral).
* tRPC `--jobs auto` (shipping default) SHA `4d69a2d5…5ee59`, 2,190 diagnostics —
  unchanged. `SURGE_DEFER=1` byte-identical across 8 consecutive runs, no crash
  (with the safe tri-state short-circuit).
* zod / ky / ofetch: `--jobs auto` == `SURGE_DEFER=1 --jobs 8` == plain
  `--jobs 8`, all byte-identical.
* Default path bytes are unchanged, so oracle parity is preserved by
  construction.

## Env probes

`SURGE_DEFER` (enable the reservation table + requeue + deferral counting),
`SURGE_REPLAY_OFF` (serial recheck, for A/B), `SURGE_STC_STATS`
(`[stc]` / `[stc-replay]` / `[stc-defer]` lines), `SURGE_REPLAY_DAG` (conflict
level histogram). Pinned: display-inclusive canonical identity (`c46f0f2` — do
not weaken).
