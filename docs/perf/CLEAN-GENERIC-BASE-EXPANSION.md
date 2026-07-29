# Clean generic base expansion: distributive-conditional member guards (2026-07-29)

## Problem

Re-enabling qualified heritage (`interface X extends NS.Base`) is blocked by a
degraded-resolution explosion (zod `interface_resolution_degraded_count`
60,961 → 147,132; +77% wall / +37% peak), and degraded results are uncacheable
by invariant. This batch attacks the *degradation itself*: why do the hot
base/interface expansions finish `had_error` at all?

## Root-cause traces (per-origin `SURGE_TRACE_HAD_ERROR=1` instrumentation)

Aggregating every `had_error` origin on zod (heritage applied, `--jobs 1`)
split the 147k degraded resolutions into two leaf families:

1. **Check phase (~55k events): unbound `infer` captures.**
   Trace (zod v3, `zod@3.24.3/lib/types.d.ts` and v4 `core/util.ts`):
   `MakeReadonly<T> = T extends Map<infer K, infer V> ? ReadonlyMap<K, V> : …`
   distributes over a concrete `T`. Two member classes wrongly selected the
   true branch:
   - a member that resolved to `any` — `is_assignable_to(any, …)` is always
     true, so the branch was taken with `K`/`V` never bound (6,764 traced
     `member_shape=Any` events);
   - a member behind a lazy reference that *peels* to the `unknown`
     degradation sentinel (`output<T>` / `input<T>` whose body could not
     resolve) — the existing "cannot decide" guard only matched a syntactic
     `Type::Unknown`, and the sentinel is assignable to everything (2,700+
     traced `member_shape=Ref` events).
   The true branch then resolved `ReadonlyMap<K, V>` with `K`/`V` as plain
   type names: a silent lookup miss (`had_error`, no diagnostic in most
   windows; two real surge-only TS2304s at `v3/types.ts(4856)`), and 9,474
   eager degraded `ReadonlyMap` expansions (54k function types per run) whose
   taint propagated up the `$ZodTypeInternals` → `$ZodReadonlyInternals` →
   `core.$ZodReadonly` base chains at every use site.

2. **Analysis phase (~281k events): import-less declaration scopes.**
   Trace (zod v4 `classic/schemas.ts`): every one of the 14,330 `core.output`
   misses carried `scope_installed=true file_in_map=false check_phase=false` —
   the declaration's pre-attached `resolution_scope` holds only the local
   layer (no namespace-import members), and the authoritative
   `module_scope_by_file` fallback is deliberately absent in the preliminary
   analysis windows. This family is superseded-round work; see "Rejected
   attempt" below for why the check-phase tail of it stays unfixed this batch.

## Fix (accepted): two member guards in the distributive conditional

`crates/surge-ts-checker/src/infer/types/resolve/conditional.rs`, distributive
loop only:

- an `any` member degrades to an open `any` — the same rule the
  non-distributive path already applies (tsc yields the union of both
  branches; surge's documented modeling keeps it open);
- a `Type::Reference` member is peeled before the branch test, and a peel to
  the `unknown` sentinel (or `any`) gets the existing "cannot decide" /
  `any` treatment instead of matching the branch.

No caching change, no diagnostic suppression: a genuinely unresolved member
still reports its TS2304 (pinned by
`distributive_conditional_unresolved_member_still_reports_ts2304`), and a
concrete `Map` member still binds `K`/`V` and keeps the true positives of the
resulting `ReadonlyMap` instantiation (pinned by
`distributive_conditional_concrete_map_member_binds_infer_captures`). The
degradation itself is pinned by a counter assertion
(`any_member_produces_no_degraded_interface_resolutions`, a unit test next to
the fix): the forced-peel fixture went from 1 degraded / 24 interface
resolution attempts (pre-fix binary) to 0 / 13.

Known, deliberate tsc divergence: tsc types a conditional over `any` as the
union of BOTH branches with unmatched `infer`s bound to `unknown` (probe:
`MakeReadonly<any>` = `Readonly<any> | ReadonlyMap<unknown, unknown>`), so on
a synthetic shape that assigns that union somewhere incompatible tsc reports
TS2322 while surge's `any` collapse stays silent. This mirrors the
non-distributive `any` rule the file already documents; exact parity here
would require branch-union materialization with `unknown`-bound captures, a
larger change than this batch justifies. On the real corpora the guards
removed only diagnostics the pinned tsc does not emit.

## Diagnostic effect (adjudicated against the pinned tsc 7.0.2)

All changes are false-positive removals; tsc reports **none** of these
locations, and no tsc diagnostic disappears:

- zod 912 → 909: −2 TS2304 (`'K'`/`'V'`, `v3/types.ts(4856)`), −1 TS2322
  (`v4/classic/from-json-schema.ts(655)`).
- tRPC 1878 → 1872: −1 TS2304 (`'U'`, `types.ts(185)`), −2
  (`createTRPCClient.ts` TS2353/TS2339), −3 (`procedureBuilder.ts` TS2322s).
- ky / ofetch / unnamed: byte-identical.

## Rejected attempt (documented for the next frontier)

Repairing the stale environment snapshots (recovered declaration environments
whose captured `module_scope_by_file` is empty got the program-published
authoritative map, check-phase-gated) removed a further ~20k degraded
resolutions (heritage build: 147,132 → 127,108) and three more corpus FPs,
but introduced **three new false positives**, all in dependency `.d.ts`
resolution: `zod packages/bench/metabench.ts(134)` TS2367 (`'Task'` vs
`'Task'`), `trpc examples/minimal-react .../App.tsx(7)` TS2322
(`'QueryClient'` vs `'QueryClient'`), and `trpc examples/nuxt .../smoke.test.ts(6)`
TS2339 (`toHaveText` on `MakeMatchers<void, T, ExtendedMatchers>`). The
stricter resolution exposes a latent two-copy nominal-identity/dedup gap (the
same declaration reached through two package copies no longer unifies once
both sides resolve concretely). The repair is sound only after that identity
gap is closed; it was fully reverted.

## Performance (accepted batch, interleaved A/B, release, cold process)

| workload | pairs | wall A median | wall B median | Δ | signs (B faster/slower) | peak A | peak B | Δ |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| zod `--jobs auto` | 11 | 2.300 s | 2.260 s | **−1.74%** | 11/0 | 645.7 MB | 638.1 MB | −1.16% |
| tRPC `--jobs 1` | 5 | 6.250 s | 6.260 s | +0.16% | 2/2 | 1,530.0 MB | 1,527.3 MB | −0.18% |
| tRPC `--jobs auto` | 7 | 5.750 s | 5.740 s | −0.17% | 5/2 | 1,544.5 MB | 1,540.0 MB | −0.29% |

(An initial 7-pair zod run was contaminated by ambient load — ranges
2.31–3.96 s vs 2.37–5.76 s — and was re-run at 11 pairs per the noise rule.)

Counters (`--jobs 1`, `SURGE_TIMINGS=1`): zod degraded interface resolutions
60,961 → 58,993 (attempts 162,686 → 159,835); tRPC 69,383 → 68,647. The
guarded family explodes mainly *under qualified heritage*, where the batch
cuts degraded resolutions 147,132 → 126,517 (−14%) and attempts
292,888 → 266,665.

## Qualified heritage re-evaluation (still rejected)

With the batch accepted as the new baseline, the parser-side heritage patch
was re-applied and re-measured (2026-07-29): fixture
`interface-qualified-heritage-basic` at exact 8/8 file/code/line parity; zod
909 → 538 (−383 FP, +12 surge-only FP, zero tsc TPs lost — the one
location-collision candidate, `assignability.test.ts(167,9)`, keeps its tsc
TS6133 and only loses a co-located surge-only TS2322); tRPC 1872 → 1863
(−13 FP, +4 surge-only FP, zero TP loss); ky/ofetch/unnamed byte-identical.
Interleaved A/B (7 pairs): zod `--jobs auto` 2.270 s → 3.610 s (**+59.0%**,
0/7), peak 639 MB → 804 MB (**+25.9%**). Better than the pre-batch
+77%/+37%, but far outside the ≤5%/≤5% acceptance gate, so heritage was
reverted again; the fixture stays unregistered. The dominant remaining
degraded volume is the analysis-phase import-less-scope family, whose sound
repair is blocked on the dependency-declaration identity gap above.

## Instrumentation kept

`SURGE_TRACE_HAD_ERROR=1` (read once per process, zero cost when unset)
prints every `had_error` origin: lookup misses (with scope/phase provenance),
interface base/member/argument taint, alias cycles/arguments, conditional
check/extends failures, and unbound `infer` captures with the member shape.
This is the instrument that localized both families; it stays for future
degradation work.
