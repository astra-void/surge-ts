# tRPC 5-second program — engineering report

Branch `perf/trpc-5s` (worktree `../surge-ts-trpc-5s`), started from `main` at
`TRPC_5S_BASELINE_COMMIT = d0e1b4cb2ff7333d0f792fd5e8a5b3288af7c3dd`.

## Outcome summary

| | baseline (d0e1b4c) | final | Δ |
|---|---|---|---|
| jobs=1 median wall | 20.52 s | **15.78 s** | −23.1% |
| jobs=auto median wall | 20.04 s | **15.51 s** | −22.6% |
| peak phys footprint (median of 5, `/usr/bin/time -l`) | ~2.01 GB | ~2.02 GB | unchanged (±noise) |
| peak phys footprint (internal `fp_peak`, isolated run) | — | 1.90 GB | |
| finish phys footprint | ~565–590 MB | **592 MB** | unchanged |
| diagnostics | 2,190; sha256 `4d69a2d5…` | 2,190; sha256 `4d69a2d5…` | **byte-identical** |

**The 5-second mission was NOT reached.** Required gates (jobs=1 ≤ 8.0 s,
jobs=auto ≤ 6.0 s) were not met. Every remaining wall-time owner is quantified
below, together with measured evidence for why the two structural levers that
would close the gap — parallel/deduplicated module analysis and parallel
checking — are blocked by the byte-identity requirement, and what designs
would unblock them.

All correctness gates PASS:

- workspace tests 1546/1546 (nextest)
- **full oracle sweep 97/97** (registry grew from 83 to 97 presets since the
  program was scoped; zero code-count or file/code/line mismatches;
  messageDriftOnly=5 is the pre-existing non-gating drift)
- real-project regressions 28 pass / 0 fail / 3 conditional skips, including
  the `unnamed` false-positive watermark
- tRPC/zod/ky/ofetch byte-identical vs the baseline binary
  (`cmp` on raw `--format json` stdout; zod 913, ky 0, ofetch 6 diagnostics)
- jobs=1 ≡ jobs=auto byte-parity; 10/10 repeated-run determinism (single
  canonical sha256 across every measured run)
- no diagnostic suppression, no permissive-Unknown fallback

**mimalloc spot-check** (`--features mimalloc`, not the gated configuration):
11.8–12.9 s wall (−24% vs system allocator), byte-identical output, peak fp
1.97 GB — directly confirming that system-allocator pressure (~22% of
self-time at baseline) is the single largest remaining constant-factor tax;
allocation-volume reduction is the highest-yield serial lever left.

## Environment

Apple M1 Pro (10 cores, 16 GB), macOS 27.0, rustc 1.94.0 (Homebrew),
node v24.12.0, pnpm 11.13.0, system allocator (xzone), default release
profile. Project: pinned tRPC checkout at `.local-projects/trpc`
(~4,933 program files), warm filesystem cache, fresh process per run,
interleaved jobs=1/jobs=auto measurement. Canonical diagnostics command:
`surge --project .local-projects/trpc/tsconfig.json --format json [--maxDiagnostics 10000] --jobs <1|auto> | shasum -a 256`.

## Final measured distribution (5 runs per mode, interleaved)

```text
jobs=1:    min 15.48  median 15.78  max 15.91  mean 15.72  σ 0.16
           user ≈ 13.6 s  sys ≈ 1.8 s
jobs=auto: min 15.35  median 15.51  max 15.66  mean 15.48  σ 0.08
           user ≈ 13.7 s  sys ≈ 1.9 s
sha256 (all 10 runs): 4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59
```

## What landed (in commit order)

1. `32d44cb` **Loader scan-once + shared specifier extraction** (−2.8 s).
   The package-declaration and import-graph scanners each fully re-parsed
   every source on every loader fixpoint iteration. A shared
   `ModuleSpecifierScanner` parses each source exactly once (keyed by its
   append-only index); both scanners resume from persistent cursors.
   package_declaration_discovery 2.50→1.10 s, import_graph_expansion
   2.23→0.68 s.
2. `b4f0ccd` **Memoized declaration resolution keys + alias ids** (−1.0 s).
   Every named-type resolution rebuilt its cache key (path canonicalization +
   two String allocations) and formatted a `"file\0name"` alias id; ~6M
   canonicalize calls/run, allocator ≈ 22% of self-time. Keys and alias ids
   are now `OnceLock`-memoized on `TypeAliasInfo`/`InterfaceInfo` (reset by
   `rename_type_declaration`, the only name rewrite) and the key's `name` is
   `Arc<str>`.
3. `6cca1f4` **Package-entrypoint probe cache** (−1.0 s wall, sys 2.3→1.7 s).
   Entrypoint resolution re-probed identical candidate paths from every
   importer directory (~550K `stat` calls). Thread-local path→is_file memo,
   cleared per `Project::check`; single `metadata` call per probe.
4. `cc8a878` **`--jobs auto` plumbing fix** (auto −0.4 s).
   `Checker::jobs` clamped with `.max(1)`, silently rewriting the CLI's auto
   sentinel (0) to forced-serial 1 — the work-based automatic sizing was dead
   code and auto never parallelized anything. Auto now fans out the parse
   phase; the check phase deliberately stays serial under auto (see blocked
   levers). Includes the `SURGE_CHECK_CACHE_ISOLATION` probe.
5. `7e9e7b8` **Parallel specifier-scan prefetch** (−0.15 s).
6. `24d4c04`-class **Loader cache release before checking** (−0.7 s wall,
   −80 MB peak). Scanner arena, specifier lists, BFS sets, and probe cache
   are dropped before the checker runs.

## Where the remaining 15.5 s goes (jobs=1, measured)

| Phase | Wall | Parallelizable today? |
|---|---|---|
| config + discovery + libs + path mapping | 0.35 s | mostly I/O-bound already |
| package_declaration_discovery | 0.94 s | residual: entrypoint resolution + canonicalize |
| import_graph_expansion | 0.69 s | reads parallelizable; resolution serial |
| checker: parse | 0.27 s serial / 0.08 s auto | ✅ (landed for auto) |
| checker: ambient + global collection | 0.85 s | untested |
| checker: preliminary type-binding collection | 1.5 s | likely (needs arena-ownership handoff) |
| checker: PRELIMINARY module analysis | ~3.3 s | ❌ blocked (see below) |
| checker: FINAL module analysis | ~3.3 s | ❌ blocked (see below) |
| checker: import bindings ×3 + scope builds ×4 | 0.45 s | — |
| checker: module_local_values | ~1.4 s | ❌ blocked (same coupling) |
| checker: per-file check loop | ~3.2 s | ❌ blocked (28-message coupling; STC design below) |
| teardown + render | 0.45 s | render is 7 ms; teardown backs the finish-memory metric |

## The central blocker, precisely quantified

**Rendered diagnostic text is coupled to shared-cache seeding order.** The
`SURGE_CHECK_CACHE_ISOLATION` probe (landed, env-gated) restores the six
program-wide resolution caches (`program_resolved_generic_types`,
`program_instantiations`, four `physical_interface_*` maps) after every
checked file, so each file observes exactly the analysis-end cache state:

- Diagnostic **set** is unchanged (2,190 = 2,190) — cross-file check-phase
  seeding is semantically invisible.
- **28 message texts change** — display-form only (`Exclude<Slot, "body">`
  vs its expansion; `Promise<HTTPResult>` vs `HTTPResult`;
  `ReadonlyArray<Auth>` vs `Auth[]`). Mechanism: a fresh resolution of a
  concrete library-scoped generic defers to a *nominal lazy reference*, while
  an instantiation-interner *hit* returns the raw interned expansion, so
  whether an earlier-checked file already peeled the instantiation decides
  the rendered form. The pre-existing `--jobs 8` divergence (10 messages,
  confirmed present at the baseline commit) is this same coupling raced.

Because the gate requires byte-identity to the serial baseline, **any**
scheduling change that alters cache visibility (parallel checking, parallel
analysis, lazy module_local_values, wave-based analysis) flips some of these
renders. Two fixes were attempted and measured:

- *Normalize the hit path to the deferral form* (hit ≡ miss): changed 1,496
  positions and **added 10 diagnostics** — wrapping non-object expansions in
  nominal references is semantically visible (union/narrowing paths), not
  just display. Reverted.
- *Per-declaration memo of `is_library_scoped_file`*: unsound —
  `ctx.file_kinds` legitimately differs between the program context and
  synthetic lazy-resolver contexts (467,690 divergences on tRPC, both
  polarities, +33 diagnostics, 3× slower). Reverted; documented in code.

## Unblocking designs (validated by the probe data, not yet built)

1. **Speculative-transactional checking (STC)** — byte-exact parallel check:
   freeze the six caches at check start; workers check files against the
   frozen state with per-file private overlays, recording the *miss set* per
   file; a serial committer walks files in order, publishing each file's
   insertions (first-wins) and re-checking any file whose miss set intersects
   keys inserted by earlier-committed files. Entries are first-wins and never
   mutate, so `misses(f) ∩ inserts(committed < f) = ∅` is an exact
   serial-equivalence condition. The isolation probe bounds the recheck tail
   (~28 affected messages → expected low-single-digit % of files).
   Estimated: check 3.2 s → ~1.0 s at auto.
2. **The same STC pattern for the analysis rounds** requires enumerating the
   full mutable read surface of a file's analysis (bindings, scope-fallback
   consults, degraded resolutions, augmentation insertions — the eq-probe
   already counts these: 2,078 / 592 / 472 / 0 exclusions on tRPC), which is
   substantially wider than the six caches. Not attempted.
3. **Input-equality FINAL-round skip is not worth it on tRPC** (new
   measurement, contradicts the plan's premise): `SURGE_EQ_STATS` shows 74.3%
   of files are prelim/final output-equal, but they carry only **35% of
   final-round time (1.15 s ceiling)**, and the current predictor is unsound
   (268 predicted-equal files actually differ; the sound predicted subset
   carries 0.08 s). The expensive dependency-`.d.ts` files are exactly the
   ones whose thin-import preliminary run differs.

## Rejected experiments (this program)

| Experiment | Result |
|---|---|
| Fx-hash for `type_dedup_fingerprint` | Diagnostic drift: dedup bucketing changes payload sharing; assignability is `Arc`-pointer-identity-sensitive (cycle edges). Hash must stay as-is. |
| Per-declaration `is_library_scoped` memo | Unsound (context-sensitive answer); 467K divergences. |
| Interner-hit form normalization | +10 diagnostics; nominal wrapping of non-object expansions is semantically visible. |
| Historical (pre-program, from project memory): conditional-result cache, fingerprint index, resolve_arc peel-clone elimination, topo-JIT single pass | +4–5% slower / neutral / ~2% / RSS 3.2× + FP drift respectively. |

## Counter snapshot deltas (SURGE_TIMINGS, jobs=1)

Unchanged by this program (resolution volume was not reduced, only its
per-request constant cost): canonical function store requests 2,594,246
(unique 526,317); union handle copies 5,113,701; interface heritage
resolutions and member mappings as at baseline. The mission's Stage-1 counter
targets (interface resolutions <150K, member mappings <600K) require the
environment-keyed resolution reuse that prior measured attempts (see project
memory) found net-negative under sound gating; not re-attempted.

## Reproduction

```bash
# benchmark (canonical):
/usr/bin/time -l target/release/surge --project .local-projects/trpc/tsconfig.json \
  --format json --jobs 1 > /dev/null
# canonical hash:
target/release/surge --project .local-projects/trpc/tsconfig.json \
  --format json --maxDiagnostics 10000 --jobs 1 | shasum -a 256
# phase attribution:
SURGE_TIMINGS=1 target/release/surge --project ... --format json --jobs 1
# check-phase coupling probe:
SURGE_CHECK_CACHE_ISOLATION=1 target/release/surge --project ... --format json --jobs 1
# prelim/final equality ceiling:
SURGE_EQ_STATS=1 target/release/surge --project ... --format json --jobs 1
```
