# Loader parse hand-off, and the per-file text sweeps around it

> **Point-in-time engineering report.** It records one investigation on its
> own date and does not describe current counts or current performance. For
> the current state see [CURRENT_STATUS.md](../../CURRENT_STATUS.md); for
> benchmark methodology see [BENCHMARKS.md](../../BENCHMARKS.md).

Dated 2026-09-11. Measured on an Apple M1 Pro against `.local-projects/ofetch`
(unjs/ofetch, 802 source lines, 293 program files: 9 sources, 198 dependency
declarations, 86 physical libs) and `.local-projects/trpc`.

**These numbers came from a working tree with unrelated uncommitted work in
it**, so they are not a snapshot of any commit and must not be copied into
CURRENT_STATUS.md. What makes them usable anyway is that both arms of every
comparison come from the *same binary*, selected at run time by
`SURGE_PRESCANNED_PARSE_REUSE`; nothing else differs between them.

## What the profile said

Symbolicated release binary
(`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` into a scratch target dir),
250 ofetch runs in a loop, sampled with `xcrun xctrace record --template
"Time Profiler" --all-processes --time-limit 20s`. A single 160 ms run yields
too few samples; the loop plus `--all-processes` gave 40,000.

ofetch is almost entirely fixed frontend cost. Checking its own sources is
about 15% of the run. Inclusive shares of total CPU:

| Region | Share |
| --- | ---: |
| Module analyses (preliminary + final, two passes) | 25.3% |
| Check phase | 15.5% |
| `parse_program_files` | 12.1% |
| Loader (package discovery, specifier scan, import graph, libs) | ~17% |
| Ambient collection | 5.8% |

By leaf-symbol category: allocator 16.3%, kernel/syscall 13.3%,
memmove/memcmp 11.9%, substring search 5.5%, hashing 5.2%.

Two of those lines were duplicated work rather than necessary work.

## 1. Every source and dependency declaration was parsed twice

`ModuleSpecifierScanner` parses each file to pull its module specifiers out,
then throws the parse away; `parse_program_files` parses the identical
`(source text, file name)` pair again. The oxc parse split 2.98% (loader) /
4.38% (program) of total CPU, and the `ParsedStatement` lowering split 2.49% /
3.30% — about 5.5% of the run spent building ASTs that already existed.

The scanner now keeps its parse and `Project::check` hands the set to
`lowlevel::check_program_with_prescanned_sources`, which matches by file name
and skips the program's parse for those files. Matching is by name, not by
index, because the program splices generated default libs into its own input
list. Physical default libs are deliberately outside the loader's scan
(`lib.rs` splices them in after it), so they stay the program's to parse,
which is why the loader's parse cost was only ~68% of the program's.

Reuse is sound because `parse_source_in` is a pure function of
`(source text, file name)` and every loader push site pairs
`inputs[i].file_name` / `source_text` with `sources[i]` verbatim.

## 2. Two full-text sweeps and three path classifications per file

`parse_program_file` computed `has_export_default` and `contains_typeof` with
`str::contains`, which builds a fresh two-way searcher per call, over every
file's whole text — 5.7 MB swept twice per ofetch run. It also called
`classify_file_kind` three times per file, each call allocating a lowercase
copy of the path plus a `replace`, then running several substring searches.
Together they were 3.5% of total CPU, the bulk of the 5.5% substring-search
line. The profile does not split that 3.5% between the two sources.

The flags are now found with shared `memchr::memmem` finders built once, and
the file kind is classified once and reused. **Both flags stay textual on
purpose**: `contains_typeof` gates releasing a declaration module's local
symbols, so a `typeof` inside a comment or a string must keep those symbols
alive. Deriving either from the AST would be a semantic change, not an
optimization.

## Measured

Interleaved same-binary A/B, medians over 25 pairs (ofetch) and 9 pairs
(trpc). CPU is `RUSAGE_CHILDREN` user+sys, which is far less sensitive to
machine load than wall time. **The table isolates change (1) only**, since
that is what the flag switches; (2) is unconditional. See "What (2) is worth"
below for why it was not given an end-to-end number of its own.

| Workload | Wall | CPU | Peak footprint |
| --- | ---: | ---: | ---: |
| ofetch | −0.9% (noise) | **−9.9%** | −7.4% |
| trpc | see below | **−6.7%** | ±1% (noise) |

**The wall-clock gain is small and the CPU gain is not.** The removed work sat
in the parse phase, which is already spread across a worker pool, so taking
CPU out of it barely shortens the critical path. On ofetch the effective
parallelism of the whole run is about 1.0 — the parallel phases are only ~20%
of the work — so this shows up as headroom rather than latency. trpc's wall
readings moved in both directions across rounds depending on machine load and
should be read as neutral; only the CPU column is trustworthy under
contention.

Peak physical footprint fell on ofetch because the ASTs used to exist twice,
once per parse, and mimalloc did not return the first set promptly. It is not
a new retention: see the prescanned row in
[MEMORY_REGIONS.md](../../crates/surge-ts-checker/MEMORY_REGIONS.md).

## What (2) is worth

Not separately isolated end to end, and the honest reason is that the working
tree moved underneath the measurement: a before/after against a binary built
earlier in the session would be comparing unrelated checker edits as well.

What a re-profile of the changed binary does show is that the sweeps
themselves stopped being hot: `memmem` frames account for 0.02% of samples,
where the two `str::contains` calls previously shared a 3.5% line with the
classifications. The substring-search work still visible inside the parse
closure is path classification
(`to_ascii_lowercase` plus several `contains` on the file name), now paid once
per file instead of three times. That re-profile is a thinner sample (9,086
against 40,000) with a different phase mix, so it is good for "is this symbol
still hot" and not for a delta.

## Gates

Oracle preset sweep `--all --maxDiagnostics 200`: 189/189, `onlyTsc` 0,
`onlyRust` 0, no file/code/line or code-count mismatches. Oracle harness
tests 21/21. Workspace tests 1868/1868. Diagnostics byte-identical between
the two arms on zod, ky, ofetch, trpc and tanstack-query.

The sweep's single `messageDriftOnly` target is identical with the change
disabled, so it belongs to other work in the tree, not to this change.

## 3. Path classification allocated on a per-type-resolution path

`classify_file_kind` lowercased the whole path into a fresh `String` twice and
built a third with `replace('\\', "/")`, then ran several two-way substring
searches over it. `is_library_classified_file_name` asks for it during
interface resolution, so that is per-resolution, not per-file. The predicates
are now allocation-free byte comparisons, matching what
`is_physical_default_lib_file_name_uncached` already did next door and for the
same recorded reason. `file_kind_tests` covers the behaviour.

## Not taken

Kept here with their measurements so they are not re-proposed.

- **A specifier-only extraction in the loader**, skipping `ParsedStatement`
  lowering and leaving the program to parse everything itself. Rejected in
  favour of the hand-off: it recovers only the lowering half (~2.5% of CPU
  against ~5.5%) and leaves every AST allocated twice.
- **Memoizing `classify_file_kind` per thread** by full file name, the way the
  default-lib name flags next to it are memoized. **Measured loss**: tRPC
  +1.6% CPU and +5.1% wall with the memo on, ofetch neutral. Once the
  predicates stopped allocating, hashing a ~120-byte path costs more than
  classifying it does. The memo is the obvious next idea after change (3), so
  this is the number that should stop it.
- **Extending lazy dependency signatures to the physical `lib.*.d.ts` set.**
  The gate in `checks/function/mod.rs` admits only
  `FileKind::DependencyDeclaration`, which looks like an oversight: the libs
  are the largest declaration surface in any program and a project uses a
  small slice. It is diagnostically free — byte-identical on zod, ky, ofetch,
  tRPC and TanStack Query, and green on the preset sweep — and still a
  **measured loss**: tRPC +1.7% CPU and +4.4% wall, ofetch neutral. Building
  lazy references for the whole lib surface costs more than the deferred
  mapping saves.

## Still open on this workload

Named here so they are not re-derived from scratch:

- Module analysis runs twice over all 293 modules and is 25% of CPU. The
  parallel variant exists behind `SURGE_PARALLEL_ANALYSIS` and buys nothing
  here (+1.3%, measured).
- `release_free_memory` (a mimalloc collect at each generation boundary) is
  ~0.65% of CPU plus whatever decommit it triggers, on a project with nothing
  to reclaim. Gating it on program size would need a threshold, and the
  measured prize is about 1%, so it was left alone rather than guessed at.
- 6,412 path canonicalize calls (743 syscalls) and 1,292 existence probes;
  `realpath` alone is 1.4%.

## Reproducing

`pnpm run real:ofetch` is the harness entry. For a like-for-like comparison
against `tsc`, note two traps: ofetch's own `tsconfig.json` sets
`esModuleInterop=false`, which TypeScript 7 reports as a removed option and
then **exits without checking** (`Check time: 0.000s`), and `composite` plus a
stale `tsconfig.tsbuildinfo` makes it reuse a previous build. Copy the config,
drop `esModuleInterop` and `isolatedDeclarations`, and set
`composite: false, incremental: false` to get a real check out of it.
`/usr/bin/time -p` has 10 ms resolution, which is too coarse here.
