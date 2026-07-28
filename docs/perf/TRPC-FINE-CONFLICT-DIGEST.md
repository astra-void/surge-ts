# tRPC: fine conflict digest — false staleness was the stale-replay blocker

Branch `perf/fine-conflict-digest`. Follow-up to
[TRPC-DEFERRED-RESOLUTION.md](TRPC-DEFERRED-RESOLUTION.md), which ended on the
hypothesis that the ~84 stale replays exist because reservations are seeded
from worker logs whose miss/insert sets differ from a replay's real
committed-view sets, and that the fix was reserving on real miss sets plus a
topological wave commit.

**That hypothesis was wrong, and the planned restructure is unnecessary.** The
cheap verification pass this report describes (`SURGE_DEFER_DIFF`) shows the
worker logs predict the publishes essentially perfectly; the staleness was
manufactured by the coarseness of the digest used as the validation currency.

## The classifier (`SURGE_DEFER_DIFF=1`)

Opt-in instrumentation in the commit walk (`program/mod.rs`):

- Before the replay walk, index every worker-log insert digest by its earliest
  reserving position (the digest-level reservation universe) and keep each
  position's predicted insert-digest set.
- As positions commit, record which position actually made each digest
  published, and how it committed (clean worker log / validated replay /
  inline recheck).
- For each stale replay, intersect its miss digests with `published` (the
  exact staleness condition) and classify every offending digest:
  - `unreserved` — no worker log predicted this publish. Only this class would
    be fixed by reserving on real insert sets.
  - `reserved_by_publisher` — the actual publisher had reserved it; deferral
    should have fired and did not.
  - `reserved_by_other` / `reserved_later_or_self` — reservation ownership
    mismatches.
- Additionally diff each replay/inline-committed position's real insert set
  against its worker-log prediction (`unpredicted_inserts` /
  `unrealized_predictions`).

## Measurement (tRPC, `SURGE_DEFER=1 SURGE_DEFER_DIFF=1`, jobs=8)

Baseline (coarse digest):

```
[stc]            miss_conflicts=187 dep_conflicts=35 clean=4770
[stc-replay]     submitted=222 replay_committed=80 stale=78 inline_only=64
[stc-defer-diff] stale=78 offenders=226 unreserved=0 reserved_by_other=11
                 reserved_by_publisher=212 reserved_later_or_self=3
                 publisher_kind(unknown/clean/replay/inline)=0/39/10/177
```

`unreserved=0`: the real-miss-reservation plan had no target. 212/226
offenders were reserved by the exact position that later published them — the
reservation existed, the digest matched, and deferral still did not fire. The
only mechanism consistent with all three observations (miss recorded, digest
published, exact-args reservation query missed) is **digest-equal but
`Type`-unequal arguments**: the replay's lookup missed the live bucket by
exact-`==`, its miss digest collided with the published entry's digest, and
the reservation's exact-args match failed the same way the bucket lookup did.
Serial checking at that position would also have missed and recomputed — the
replay was valid, and the digest-set validation could not see it.

## Why the digest collided

`type_conflict_digest` was `dedup_key`: depth-3 cutoff, object types hashed by
property names only (no types), and `Type::Function(_) => {}` — function types
contributed nothing beyond the enum discriminant. Any two instantiation
argument lists differing only in a function signature, an object property
type, or anything below depth 3 collided.

## The fix

`type_conflict_digest` gets its own walker (union dedup's `dedup_key` is
untouched): budgeted full-structure hashing (node budget 64, depth 8),
function parameters/return/variadic/required-count, object property types
folded commutatively under independent per-property budgets of 32
(order-independent `IndexMap` equality means a shared budget would truncate
order-dependently and break equality consistency), and the interned
`TypeListId`s carried by function/union payloads (equality-participating, and
a full-depth list fingerprint in one `u64`). The consistency invariant is
unchanged: hash only (a subset of) equality-participating fields,
structurally, never by pointer. A finer digest is strictly sound — validation
only requires `a == b ⇒ digest(a) == digest(b)`.

Budget calibration (tRPC, interleaved): 256/16/64 and 1024/24/128 give the
same stale counts as 64/8/32 — the interned list-id fingerprints carry the
deep discrimination, so the walk stays shallow. The larger budgets cost
+0.15–0.2 s of worker phase and inflate the commit walk (digests are also
computed inside inline-recheck and replay sessions); 64/8/32 is at hashing
parity with the coarse baseline.

After (same configuration):

```
[stc]            miss_conflicts=153 dep_conflicts=32 clean=4807
[stc-replay]     submitted=184 replay_committed=91 stale=25 inline_only=69
[stc-defer-diff] stale=25 offenders=44 unreserved=0
```

- Stale replays 78 → 25; offender digests 226 → 44.
- Predicted conflicts 222 → 186, clean commits +37 — the *default* (non-defer)
  pipeline also validates more worker logs first-try.
- Raising budgets to 1024/24/128 bought nothing (stale 27) — the residue is
  not truncation. The remaining ~25 look like the commit/query race window
  (miss recorded, publisher commits and finalizes, reservation query then sees
  `Ready`) plus reservation-ownership mismatches; diminishing returns.

## Gates

- Byte-identity: 4 corpora (trpc/zod/ky/ofetch) × {jobs auto, 8, 1,
  8+SURGE_DEFER} × {baseline, fine} — identical output hashes for both the
  256/16/64 and the final 64/8/32 constants (16/16 each per binary pair).
- `cargo nextest run --workspace` — 1579/1579 green.
- Oracle sweep `--all --maxDiagnostics 200` — 97/97, 5 message-drift-only
  (unchanged baseline).

## Phase-time A/B (interleaved, same window)

Default parallel pipeline (`--jobs 8`, no SURGE_DEFER), base vs final 64/8/32,
4 interleaved rounds:

```
              worker_phase    commit_phase    miss_conflicts   stale
base          0.78-0.82 s     1.43-1.82 s     184-193          16-17
fine 64/8/32  0.81-1.03 s     1.05-1.45 s     142-156          0-5
```

Commit phase is lower in every round pairwise (median 1.58 → 1.30 s); stale
replays on the dependency-driven schedule collapse to ≈0. Under SURGE_DEFER
(eager schedule), commit phase 1.90 → 1.39 s median over 3 interleaved rounds.

Scope note: `--jobs auto` keeps the check phase serial (`resolve_worker_count`),
so the shipping default neither pays nor gains anything here — the digest is
only computed inside speculative sessions. The win applies to explicit
`--jobs N` and to any future flip of AUTO to the parallel path; by shortening
the recheck tail it moves that flip's parity argument in the right direction.

## Where this leaves the deferred-resolution program

- The 3.65× critical-path ceiling (`SURGE_CRITPATH`, serial_sum ≈1.7s vs
  critical_path ≈0.46s) still stands, but the serial tail it applies to is now
  ~100 inline recomputes rather than ~140, and the false-staleness channel no
  longer forces valid replays back onto the coordinator.
- Reserve-on-real-miss-sets is dead as a lever (`unreserved=0` both before and
  after). Topological wave commit remains the structural option for the
  residual inline tail, but the earlier measurements (out-of-order pool
  round-trips ≈2× worse; latest-dep scheduling +35%) still apply — the
  coordinator-blocking cost model is unchanged.
