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

## The blocker, precisely

The value dependency between conflicts is shallow (~6 truly-divergent conflicts),
but the *digest* dependency DAG is deep (max level ~35). The over-recursion
representation is now handled safely, yet the tail does not get faster because
the residual cost is **scheduling**, not resolution: stale conflict-dependent
replays and pool/coordinator contention. The remaining levers are orthogonal to
deferred resolution:

### Next concrete changes

* **Cut stale replays.** Schedule conflict-dependent replays at their latest
  worker-log dependency (drop the eager-at-0 submit) so fewer run against an
  incomplete view; or reserve on the *replay's real* miss set rather than the
  worker log.
* **Make the short-circuit terminal without re-expanding.** Return a nominal that
  resolves to a cached structural placeholder (not Unknown) so a forced deferred
  nominal never over-recurses even once — removing the defer-once expansion cost.
* **Address the contention ceiling** (separate, measured): the pool + coordinator
  on 8 cores. Until that moves, a faster tail is unlikely regardless of how cheap
  the deferred replays become.

Until then the serial recheck remains the fastest correct tail for tRPC. The
reservation table, the measurement, and the requeue substrate are kept as the
validated, non-regressing foundation (opt-in `SURGE_DEFER`; the shipping default
is untouched).

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
