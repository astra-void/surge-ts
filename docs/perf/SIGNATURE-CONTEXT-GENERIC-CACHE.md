# Signature-context generic-instantiation cache (2026-07-29)

## Problem

The generic-instantiation reuse paths never hit for user generics referenced
inside generic signatures/bodies — the dominant resolution context on zod and
tRPC. Per-declaration trace at the baseline (zod, `--jobs 1`,
`SURGE_TRACE_DTS_EXPANSION=1`):

- `$ZodTypeInternals` (src copy): 12,114 body resolutions, 145 unique
  canonical argument tuples, 0 cache hits.
- `ReadonlyMap`: 1,452 body resolutions, all degraded, zero canonical keys.

## Root cause (probe evidence, per-attempt classification)

The eager-instantiation short-circuit and interner in `resolve_named_type`
required `concrete_instantiation` — every type-parameter scope on the checker
stack empty. Any reference made inside a generic function signature or generic
body was excluded from both lookup and store, even when its resolved argument
tuple was concrete, and the expansion clean, diagnostic-free, and cycle-free.

Per-attempt distribution for `$ZodTypeInternals` (both copies, zod `--jobs 1`,
temporary gated probe, since removed):

| class | attempts | cacheable? |
| --- | ---: | --- |
| concrete-tier sites (existing tier's domain) | 8,350 | existing behavior |
| non-concrete, clean args, would-be store rejected only by the concrete gate | 5,591 | blocked → the fix's target |
| analysis-phase | 4,248 | no — module scopes still move between binding rounds |
| `unknown`-sentinel-carrying argument tuples | 1,225 | no — the sentinel collides across contexts |
| argument resolution errored | 303 | no |
| mid-cycle (declaration on the `resolving` stack) | 101 | no |
| syntactic placeholder arguments | 84 | no — the placeholder mark changes body resolution |

Traced repeated tuple: `ParsePayload<output<any>>` — one store on its first
clean check-phase expansion, then 601 identical attempts on the same run, each
previously re-resolving the body (plus its inheritance chain).

`ReadonlyMap` is *not* a cache bug: its only zod reference sites are inside
`util.MakeReadonly<T>`'s conditional true branch, so every instantiation
carries `infer`-placeholder (`unknown`/`any`) arguments, self-cycles, and
degrades. The cache correctly refuses all of it.

## Fix

`crates/surge-ts-checker/src/infer/types/resolve/named.rs` — a
"signature-context" tier over the existing `program_instantiations`
interner (same buckets, caps, session/replay machinery, and teardown), with a
namespace-separated key (`DeclarationNamespace::TypeSignatureContext`) so its
entries can never share a bucket with concrete-tier entries — nested
references defer differently in the two context classes, so same
(declaration, arguments) does not mean same representation.

Lookup/store eligibility (symmetric key construction):

- non-concrete site, check phase only, user (non-library) interface;
- declaration not currently on the `resolving` stack;
- full explicit argument arity (defaults resolve under an effective
  substitution that can see the consumer, so defaulted references stay out);
- no syntactic placeholder argument; no `unknown` sentinel anywhere in the
  resolved argument tuple (deep, budgeted screen; `GenuineUnknown` allowed);
- key includes a deep display fingerprint of the argument tuple —
  `Type` equality compares references by (id, arguments) only, and reusing a
  nominally-equal-but-differently-rendered tuple substitutes the first site's
  rendering into later consumers' messages (the display-substitution class).

Stores additionally require: clean (`!had_error`), no diagnostics (plain or
once-guard), cycle-free subtree, degradation epoch unchanged across the body,
and the full physical-cache value validation — embedded-`unknown` rejection
included. Two relaxations were tried and rejected with evidence:

1. Tolerating embedded sentinels in stored values (+1,259 zod hits) changed
   real zod diagnostics: four TS2339s vanished and one TS2322 render drifted —
   the sentinel-embedding shapes are exactly the ones that bake a
   first-resolution-window (thin vs. rich dependency) difference.
2. Restricting the tier to top-level resolutions (`resolving` empty) restored
   byte-identity trivially but collapsed zod hits to 13.

Kill switch: `SURGE_DISABLE_SIG_CONTEXT_CACHE=1` (read once). Counters:
`signature_context_generic_hit_count` / `_store_count` in the
`SURGE_TIMINGS` table.

## Results (accepted configuration)

Cache effect (`--jobs 1`, `SURGE_TIMINGS=1`):

| project | sig hits | sig stores | interface body resolutions | clean resolutions | degraded |
| --- | ---: | ---: | ---: | ---: | ---: |
| zod A | — | — | 173,308 | 82,783 | 61,025 |
| zod B | 7,001 | 289 | 162,686 (−6.1%) | 72,231 | 60,961 |
| tRPC A | — | — | 235,942 | 76,576 | 69,370 |
| tRPC B | 2,377 | 171 | 233,122 (−1.2%) | 73,732 | 69,383 |

The attempt drop exceeds the hit count because each hit also skips the nested
base-chain re-resolution (`$ZodTypeInternals <- _$ZodTypeInternals` alone was
~206k inherited-member merge units per zod run).

Interleaved A/B, release binaries, same machine, one warmup per binary, cold
process (`/usr/bin/time -l`, peak = phys footprint):

| workload | pairs | wall A median | wall B median | Δ | signs (B faster/slower) | peak A | peak B | Δ |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| zod `--jobs auto` | 7 | 2.170 s | 2.160 s | −0.46% | 4/2 | 673.5 MB | 645.3 MB | **−4.18%** |
| tRPC `--jobs 1` | 5 | 5.620 s | 5.640 s | +0.36% | 1/3 | 1,532.3 MB | 1,532.2 MB | −0.01% |
| tRPC `--jobs auto` | 11 | 5.430 s | 5.430 s | +0.00% | 4/7 | 1,539.8 MB | 1,544.1 MB | +0.28% |

The tRPC-auto footprint delta is a consistent ≈+4 MB absolute (9/11 pairs) —
the retained signature-context entries — well inside the ±30–50% noise
documented for tRPC RSS and the 2% acceptance bound. The zod footprint drop is
consistent (7/7) — shared expansions replace per-site private copies.

Environmental caveat: single desktop machine, ambient load not controlled
beyond interleaving; deltas, sign counts, and counters are the meaningful
signal, not the absolute times.

## Validation

- Diagnostics byte-identical to the baseline on zod / tRPC / ky / ofetch /
  unnamed at `--jobs auto`, and across `--jobs 1/2/4/8/auto` (zod, tRPC).
- `cargo nextest run --workspace`: 1591/1591 (10 new tests: reuse parity,
  tuple/file collision negatives, recursion, degraded-not-frozen,
  placeholder-args, bounded cap, cross-program teardown, hit/store counters).
- Oracle: `oracle:test` 21/21; sweep 98/98, `messageDriftOnly=5`,
  `spanDriftOnly=0` (both unchanged).
- `real:test`: 29 pass / 2 pre-existing ky failures (identical failing set
  against the baseline binary via `SURGE_TS_BIN`).
- `bench:complexity` PASS (incl. two-process determinism hashes);
  `bench:test` 62/62.

## What this did *not* fix, and the next frontier

The analysis-phase re-resolutions (uncacheable while scopes move) and the
degraded expansions remain per-site. The qualified-heritage patch
(`extends NS.M`) was re-evaluated on top of this cache and is still
categorically too expensive (zod +77% wall / +37% peak, 7/7 pairs): its cost
is *degraded* re-resolution volume (zod 60,961 → 147,132), which no sound
cache may absorb. The lever for that batch is making the newly-resolved base
expansions clean in generic contexts, not caching them.
