# tRPC 5-second mission — session report (2026-07-18, part 2)

Branch `main`, session commits `46a02de..d45de1f` on top of `3375bc6`.
Hardware: Apple M1 Pro (10 cores), 16 GB, macOS 27.0, rustc 1.94.0, release
profile. Fixture: `.local-projects/trpc` @ `3e0e9793eb7f8c4cfbe70a1dccb72f8d355e3c8b`.
Canonical command:
`target/release/surge --project .local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000 --jobs auto`.

Mission gates: wall ≤ 5.99 s median, peak physical footprint < 1.50 GB at
default `--jobs auto`, byte-identical diagnostics across `--jobs
1/2/4/8/auto` (tRPC sha `4d69a2d5…`, 2,190 diagnostics), full regression
gates.

## Result summary

| gate | start of session | end of session | status |
|---|---|---|---|
| wall (auto, 5-run median) | 13.62 s (13.45–15.72) | **10.59 s** (10.53–10.69) | not met (≤ 5.99) |
| peak fp external (auto) | 1.95–2.11 GB | **1.97 GB** (1.970–1.979) | not met (< 1.50) |
| internal fp peak | 2.02 GB | 1.973 GB (matches external) | — |
| finish fp | ~0.55 GB | 1.77 GB shown at finish-stage entry; 0.55 GB after teardown | — |
| diagnostics | 2190 / `4d69a2d5` | 2190 / `4d69a2d5`, jobs 1/2/4/8/auto raw-identical | **met** |
| zod/ky/ofetch jobs=1 ≡ auto | held | held (raw `cmp`) | **met** |
| serial `--jobs 1` regression | — | 13.66 s → 11.26 s (improved), bytes unchanged | **met** |

Validation at final HEAD: `cargo fmt --check` clean, `cargo check` clean
(via build), `cargo nextest run --workspace` 1555/1555 (three separate runs
during the session), `pnpm oracle:test` green, full oracle sweep
`--all --maxDiagnostics 200`: **97/97 pass, 0 gating mismatches, 5
message-drift-only** (non-gating), `pnpm real:test` 31/31.

## Landed commits

- `46a02de` fix(stc-analysis): close residual speculation channels in the
  commit walk — utility-diagnostic-key speculation (validated + merged in
  file order), recheck `environment_attempt` kept at 0 (serial-exact
  environment identity; the discarded attempt publishes nothing so identity
  collision is harmless), last-module-only memo capture + conflicted-log
  drop (discarded-attempt liveness in the weak store), cheap per-module
  worker seeds (~2× faster parallel passes), hunt probes.
- `c46f0f2` fix(types): display-inclusive canonical store identity — **the
  keystone**. `canonical_types_equal` ignored `Reference.display` and
  `Object.alias_name/alias_id`, so intern hits substituted display variants
  and rendered bytes depended on program-wide intern order (unrollbackable,
  schedule-dependent under any parallelism, invisible to STC validation).
  Display fields now participate in canonical identity. trpc/ky/ofetch
  serial bytes unchanged; zod changes in exactly one TS2322 render
  (`IterableIterator<T>` → `<K>`, the lib's declared parameter — the old `T`
  was itself a substitution artifact).
- `c11d2f9` perf(analysis): final-pass parallelization + declaration-module
  worker eligibility (still opt-in via `SURGE_PARALLEL_ANALYSIS`);
  `declare global` block modules stay serial (first-wins value publication);
  `SURGE_STAGE_TIMES` (stage timeline without `SURGE_TIMINGS`' per-file
  mutex traffic, which serializes parallel phases badly enough to invert
  measurements).
- `4c487a4` perf(cli): mimalloc default allocator — serial 14.16 → 11.0 s at
  1.96 vs 2.08 GB peak fp, byte-identical (interleaved A/B).
- `d45de1f` perf(stc): release fan-out snapshots and committed logs eagerly.

## Byte-identity: what was broken and how it was found

Full parallel module analysis (both passes, declaration modules included) is
now **byte-identical to serial on all four corpora at jobs 2/4/8, ten
consecutive zod runs, and under the adversarial per-module-session probe**
(`SURGE_ANALYSIS_MODULE_SESSIONS=1`, which maximizes snapshot misses).

Channels closed this session, in discovery order:

1. **Utility-diagnostic keys** (tRPC +2 TS2304 `'Options'`): serial analysis
   records `push_utility_diagnostic_once` keys even when the diagnostics are
   truncated, and the check phase depends on those keys for suppression.
   Worker key sets were discarded on clean commit. Localized by a
   worker-dispatch range bisection (`SURGE_ANALYSIS_PAR_RANGE`) to module
   1248 — `hooksToOptions.ts` itself, clean-committing yet diverging.
2. **`environment_attempt` skew**: rechecks ran attempt=1, shifting every
   environment identity (and all downstream cache keys formed from
   prelim-created environments in the final pass) away from serial.
3. **Discarded-attempt liveness**: every worker outcome retained its module's
   memo map, and conflicted files' insert logs were kept through the walk —
   both held speculative intermediate expansions strongly alive, so serial
   rechecks intern-hit discarded display variants in the weak canonical
   store instead of recomputing serial forms.
4. **Canonical-store display substitution** (zod, 1 TS2322 flip): the root
   class. Proven by regime experiments: fresh context + live cache view ≡
   serial; snapshot view diverged with *zero* observation-log difference
   (389 misses, 1,770 published, empty intersection), and per-pass product/
   insert display-fingerprint dumps (`SURGE_ANALYSIS_PRODUCT_PROBE`)
   localized value divergence to lib `Set`/`IterableIterator` instantiation
   entries. No recheck can fix store-display races (the store is shared and
   unrollbackable) — the identity had to become display-inclusive.

## Where the remaining time and memory are (measured, parallel opt-in, system-allocator numbers)

Stage timeline (`SURGE_STAGE_TIMES=1`, `--jobs 8`,
`SURGE_PARALLEL_ANALYSIS=1`, no other instrumentation):

| phase | wall | notes |
|---|---|---|
| frontend (before program start) | ~2.4 s | serial BFS: package resolution + import-graph expansion + reads |
| ambient + global collection | 0.72 s | serial |
| preliminary module analysis | 3.78 s | worker 0.88 s + ordered commit 1.68 s + ~1.2 s serial binding/scopes/imports around the driver |
| final module analysis | 2.03 s | worker 0.93 s + commit 0.84 s |
| module binding + release | 0.19 s | |
| module_local_values | 1.22 s | per-module, `&ctx` reads, cache writes unsessioned |
| check phase | 2.98 s | worker 0.93 s + commit/recheck tail 2.03 s |
| finish | 0.59 s | teardown |

With mimalloc, everything shrinks ~20 %: default auto lands at 10.59 s
median; opt-in parallel analysis at jobs 8 lands at ~10.25 s but costs
+0.6 GB peak (2.59 GB) — which is why **auto keeps the serial-analysis
path**: 0.26 s is not worth +0.6 GB against a 1.50 GB gate.

Memory: serial peak fp is 1.96–1.98 GB. Historical retained-owner census
(MEMORY-OPTIMIZATION-REPORT): export tables ~257 MB, module local symbols
~98 MB, parsed declaration bodies ~155 MB, root ASTs ~150 MB, declaration
environments ~80 MB, instantiation caches ~64–77 MB. The parallel adder
(~0.6 GB) is dominated by per-worker `CheckerContext` clones in the check
phase plus in-flight worker products.

## The remaining path (ordered by measured leverage)

1. **Ordered-delta pipelined replay of conflict tails** (~2.5–3.5 s at j8):
   the three serial commit tails (1.68 + 0.84 + 2.03 s) re-run conflicted
   files one at a time. A round-based parallel replay is sound only with
   *versioned committed deltas*: sessions must record consumed-delta
   publishers and validate **hits** as well as misses (a later-round recheck
   otherwise sees future publications; and any committed file `j > k` was
   validated against a view that could not contain conflicted `k`'s
   publications — so re-validation of `j` against `k`'s eventual
   publications, or positional filtering by publisher stamp, is mandatory).
   The display-identity fix (c46f0f2) removes the store-display obstacle
   that would previously have made any replay unsound.
2. **Frontend** (~1.5–2 s): the loader loop is a serial BFS; specifier
   scanning and file reads parallelize; package resolution needs a
   concurrent cache.
3. **module_local_values** (~1 s at j8): per-module independent but
   annotation resolution writes the six caches — run it under the existing
   `CheckSession` + ordered-commit machinery.
4. **Binding fixpoint / prelim non-driver serial work** (~1.4 s): dirty-SCC
   tracking with versioned export surfaces (mission Stage F).
5. **Memory to 1.50 GB**: (a) compact `WorkerContext` for the check phase
   (the full-context clone is ~75 MB × workers); (b) Stage-G retained-data
   reductions — lazy export-table value graphs and earlier AST/analysis
   release are the two biggest owners. This is required even for serial
   (1.96 GB floor).

Model after 1–4 at 8 workers with mimalloc: ~10.6 − (2.5 + 1.5 + 1.0 + 1.0)
≈ **5.6–6.5 s** — the gate is reachable, but only with all four levers, and
the memory gate needs (5) besides.

## Rejected / reverted this session

- `environment_attempt`-only fixes and log-drop-only fixes for the zod flip:
  necessary but insufficient (flip persisted until display-inclusive
  identity landed).
- Fresh per-module store forks (`SURGE_ANALYSIS_FRESH_STORE` probe): broke
  pointer-identity invariants without fixing the flip; probe retained for
  hunts only.
- Flipping auto to parallel analysis/check: +0.6 GB for ≤ 0.3 s at current
  tail costs — net-negative until (1) lands.
- `SURGE_TIMINGS` for parallel measurement: per-file mutex traffic inverts
  parallel phase measurements; use `SURGE_STAGE_TIMES` + `SURGE_STC_STATS`.

## Probes added (all env-gated, inert by default)

`SURGE_STAGE_TIMES`, `SURGE_FILE_ORDER_DUMP`, `SURGE_ANALYSIS_PAR_RANGE`,
`SURGE_ANALYSIS_FRESH_RANGE`, `SURGE_ANALYSIS_FRESH_STORE`,
`SURGE_ANALYSIS_MODULE_SESSIONS`, `SURGE_ANALYSIS_DECL_SERIAL`,
`SURGE_ANALYSIS_PRODUCT_PROBE` (=index or `all`), plus
worker/commit-phase timing on `SURGE_STC_STATS` for the analysis drivers.

## Follow-up: ordered-delta pipelined replay (blocker)

The "ordered-delta pipelined replay" of the three commit tails — the lever
named as item (1) above — was implemented and validated in a later session
(`a7d9a6b`, `8c5822f`). It is sound and byte-identical but **does not reduce
tRPC wall time**: the conflicts form deep digest-dependency chains (levels up
to 42) and a replay reading an incomplete committed view over-recurses, so only
clean-dependent conflicts replay reliably and the serial conflict chain
dominates. Full analysis, the variants tried, and the concrete cache-
representation change needed to unblock it (publisher-stamped versioned entries
+ a resolution-deferred sentinel) are in
[TRPC-ORDERED-DELTA-REPLAY.md](TRPC-ORDERED-DELTA-REPLAY.md). The shipping
`--jobs auto` default is unchanged (serial check, ~10.3 s median, 1.97 GB).

## Follow-up: publisher-stamped deferred resolution (blocker)

The over-recursion representation change the ordered-delta report proposed was
attempted: publisher-stamped pending reservations + a resolution-deferred
result. The reservation primitive, a deferral measurement (≈3 000 deferrals on
tRPC), and a byte-safe file-level requeue substrate landed (opt-in `SURGE_DEFER`;
`--jobs auto` unchanged). Honoring deferral moves the structural metric (inline
rechecks 153 → 64, byte-identical) but does **not** reduce wall time (check tail
1.45 → 1.66 s): the requeue reschedules the over-recursion rather than
eliminating it, and the sub-file short-circuit that would make deferred replays
cheap requires a mid-resolution nominal return out of the total
`ResolveReference::resolve_arc` — which panicked (poisoned a resolver mutex) and
is reverted. Full analysis + the safe tri-state next step in
[TRPC-DEFERRED-RESOLUTION.md](TRPC-DEFERRED-RESOLUTION.md).
