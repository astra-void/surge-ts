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

**Outcome:** the mechanism is built, byte-safe, and measured. It **does not
reduce tRPC wall time.** Honoring deferral is byte-identical and moves the
structural conflict metric (inline rechecks 153 → 64), but the requeue overhead
plus pool/coordinator contention leave the check tail no faster (1.45 → 1.66 s);
the CPU-saving piece — a mid-resolution short-circuit that returns the nominal
form instead of over-recursing — is the **unsafe frontier**: it panicked
intermittently on a pool thread (poisoning a mutex and aborting the run) and,
even when it did not, was slower still (2.05 s). Per the mission's fallback
clause this is delivered as a blocker report; the reservation primitive, the
deferral measurement, and the byte-safe requeue substrate are kept (opt-in
behind `SURGE_DEFER`, the shipping default fully unchanged), and the unsafe
short-circuit is reverted.

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

### The short-circuit is the unsafe frontier

To make a deferred replay *cheap*, the lazy instantiation peel must, on a
deferred interner miss, return the **nominal reference un-memoized** instead of
expanding the declaration body. This is genuine mid-resolution control flow out
of `ResolveReference::resolve_arc(&self) -> Arc<Type>`, a **total** trait method
with no error channel. The implemented version — a thread-local signal set by the
session lookup, read-and-cleared by the peel, returning a fresh nominal
`Type::Reference` — **panicked intermittently on a pool thread** (2 of 3 runs
produced empty output; the surviving run was byte-identical but slower at
2.05 s). A panic mid-resolution poisons one of the resolution-path mutexes and
cascades to an aborted run. Proving that return path clean to a byte-identical
standard (no poisoned lock, no half-updated `LAZY_PEEL_STACK`, no once-guard or
canonical/substitution-store divergence) is the open work; the crashing variant
is reverted.

## The blocker, precisely

The value dependency between conflicts is shallow (~6 truly-divergent conflicts),
but the *digest* dependency DAG is deep (max level ~35). A replay follows the
digest DAG unless it can resolve a not-yet-committed dependency **without
over-recursing**. The requeue substrate lets it defer *at the file level*
(discard + re-run), which is byte-safe but reschedules the full over-recursion.
Collapsing the deep digest DAG to the shallow value DAG needs the *sub-file*
short-circuit — the nominal return — and that return must be produced without a
non-local escape that poisons the resolver. That is the remaining, genuinely hard
piece.

### Next concrete change

Make the deferred nominal return **safe** rather than a panic/return hack:

* Give the six lazy resolvers a private tri-state return
  (`Ok(Type)` / `Err(ResolutionDeferred)`) threaded only through the
  instantiation-peel call chain (`LazyInstantiation` / `LazyDeclarationAnnotation`
  → `resolve_type_alias` / `resolve_interface`), converting `Err` to the nominal
  reference at the peel boundary — no thread-local, no panic, no poisoned lock.
* Only then is the deferred replay cheap enough that requeue against the *shallow*
  value DAG can beat the serial recheck — if pool/coordinator contention (a
  separate, measured ceiling) permits.

Until then the serial recheck remains the fastest correct tail for tRPC. The
reservation table, the measurement, and the requeue substrate are kept as the
validated, non-regressing foundation (opt-in `SURGE_DEFER`; the shipping default
is untouched).

## Validation

* `cargo fmt --check`, `cargo check --workspace`: clean.
* `cargo nextest run --workspace`: green (incl. 13 reservation tests + 10 replay
  oracle tests extended for deferral).
* tRPC `--jobs auto` (shipping default) SHA `4d69a2d5…5ee59`, 2,190 diagnostics —
  unchanged. `SURGE_DEFER=1` byte-identical across 5 consecutive runs (no crash
  after the short-circuit revert).
* Default path bytes are unchanged, so oracle parity is preserved by
  construction.

## Env probes

`SURGE_DEFER` (enable the reservation table + requeue + deferral counting),
`SURGE_REPLAY_OFF` (serial recheck, for A/B), `SURGE_STC_STATS`
(`[stc]` / `[stc-replay]` / `[stc-defer]` lines), `SURGE_REPLAY_DAG` (conflict
level histogram). Pinned: display-inclusive canonical identity (`c46f0f2` — do
not weaken).
