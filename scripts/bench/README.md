# Compiler Speed Benchmark Harness

This directory contains the developer-facing regression benchmark (`compare-compilers.ts`) for measuring wall-clock performance and diagnostic drift.

It is **not** a marketing benchmark. It compares the same `tsconfig.json` inputs across:

1. JavaScript TypeScript compiler (`tsc` = TypeScript 6.x, the `typescript-6` workspace alias, kept as the slow baseline)
2. TypeScript native compiler (`tsgo` = TypeScript 7.0, the canonical `typescript` package)
3. `surge-ts` CLI (using the release binary, not `cargo run`)

## Requirements

1. **Release Binary Required:** The benchmark tests the prebuilt release binary, not debug builds. Build it first:

    ```bash
    cargo build --release -p surge-ts-cli
    ```

2. **`tsgo` is Included When Available:** the native TypeScript 7.0 compiler ships as the workspace `typescript` package, so no global install is needed. If it is resolvable, the harness benchmarks it automatically and still cleanly skips it when it is unavailable.
3. **TS 7-Oriented Fixtures:** Committed fixtures under `tests/compat-projects/` must not use `ignoreDeprecations` in their `compilerOptions`. If a fixture triggers a TS 6 deprecation warning, fix the fixture config instead of suppressing the diagnostic. The harness actively guards against `ignoreDeprecations` usage.

## What It Measures

The benchmark compares **no-emit project checking** only:

- `tsc --noEmit --pretty false --project <tsconfig>`
- `tsgo --noEmit --pretty false --project <tsconfig>`
- `surge-ts --project <tsconfig> --format json --maxDiagnostics 10000`
- `surge-ts --project <tsconfig> --format json --maxDiagnostics 10000 --jobs <auto|n>` for deterministic project-checking measurements; `auto` (the default) sizes workers by cores and workload, or pass an explicit count

It does not measure watch mode, incremental builds, project references, emitting, or editor performance.

**Diagnostic Drift:** Speed is meaningless if the diagnostic surface is wrong. The script runs a single baseline check to capture diagnostics for all tools and reports diagnostic drift (e.g. `exact vs tsc`, `known delta`, or `parse failed`) alongside median timings.

## Usage

Run a specific project:

```bash
pnpm run bench:compilers -- --project tests/compat-projects/optional-chaining-basic/tsconfig.json
```

Run a preset:

```bash
pnpm run bench:compilers -- --preset current
```

Run a real project checkout (directory inputs resolve to their `tsconfig.json`):

```bash
pnpm run bench:trpc          # shorthand for the command below
pnpm run bench:compilers -- --project .local-projects/trpc --json .bench/compilers/trpc.json --chart .bench/compilers/trpc.svg --html .bench/compilers/trpc.html
```

`bench:ky`, `bench:trpc`, `bench:zod`, `bench:ofetch`, and `bench:unnamed` are
predefined for the usual local checkouts and write their JSON/SVG/HTML reports
to `.bench/compilers/<name>.*`. Extra flags append after `--`, e.g.
`pnpm run bench:trpc -- --iterations 3 --rustJobs 1`.

Change iterations, generate visual reports, and output JSON:

```bash
pnpm run bench:compilers -- --preset current --iterations 10 --warmup 2 --json .bench/compiler-bench.json --chart .bench/compiler-bench.svg --html .bench/compiler-bench.html
```

Measure Rust at a specific worker count (default is `auto`; pass `1` for serial or an explicit count to calibrate):

```bash
pnpm run bench:compilers -- --project tests/compat-projects/parallel-ordering-basic/tsconfig.json --rustJobs 4
```

Generate visual reports from an existing JSON run without re-running compilers:

```bash
pnpm run bench:compilers -- --fromJson .bench/compiler-bench.json --chart .bench/compiler-bench.svg --html .bench/compiler-bench.html
```

`tsgo` is enabled by default when available:

```bash
pnpm run bench:compilers -- --preset current
```

If you want to make the intent explicit, `--include-tsgo` remains accepted as a no-op flag.

Generate and run a synthetic scale fixture:

```bash
pnpm run bench:compilers -- --generate scale-small --files 50 --symbols 200
```

*(Generated fixtures are written to `.bench/generated/` and not committed).*

## Archiving Runs

`archive-run.ts` wraps the existing benchmark and real-project commands and saves
their raw output plus a small summary into a timestamped local directory under
`.bench/runs/<timestamp>/`. It does not change what those commands measure; it
only captures and labels their output so before/after runs are easy to compare.

```bash
pnpm run bench:archive                                  # compiler benchmark only (default)
pnpm run bench:archive -- --bench                       # explicit compiler benchmark
pnpm run bench:archive -- --real-auth-kit               # real auth-kit measurement
pnpm run bench:archive -- --bench --real-auth-kit       # both
pnpm run bench:archive -- --label builtin-removal-before
pnpm run bench:archive -- --out .bench/runs/custom-name # override the output directory
pnpm run bench:archive -- --dryRun                      # print commands + paths, run nothing
```

Each run directory contains the captured `bench-compilers.txt` / `real-auth-kit.txt`
logs (and the bench `--json`), plus `summary.json` and `summary.md` with the git
branch/commit, per-command pass/fail and exit codes, parsed benchmark medians, and
auth-kit diagnostic counts when available. A partial summary is still written when
one command fails; the process exits non-zero if any underlying command fails.

`--real-auth-kit` runs `pnpm run real:auth-kit`, which resolves the auth-kit project
from `AUTH_KIT_PROJECT` or known local paths and fails with a clear message (captured
in the log) when it is missing. `.bench` is git-ignored, so archived runs are local by
default.

## Running Benchmark Tests

To verify the harness itself:

```bash
cargo build --release -p surge-ts-cli
pnpm run bench:test
```

## Report Outputs

Every benchmark run also samples **peak memory** per iteration through the
same `/usr/bin/time` wrapper as `measure-project.ts` (macOS `-l`, Linux `-v`),
so wall time and memory come from the same invocations. On macOS the metric is
**phys_footprint** (`time -l`'s "peak memory footprint" line — the
Activity-Monitor-comparable number); elsewhere it falls back to maximum RSS.
The `source` field records which metric was used. Where peak memory cannot be
measured, timing still works and the memory fields are null.

- `--json` writes `{ meta, results }`: `meta` records the timestamp, git
  branch/commit, CPU model and core count, platform, Node version, and
  iteration/warmup counts; `results` holds per-project median/min/max/runs,
  drift, and median/min/max peak memory per tool. Legacy bare-array JSON
  files are still accepted by `--fromJson`, `bench:archive`, and
  `measure-project.ts`.
- `--chart` renders a single SVG with two stacked panels: wall time (median
  bars on a time axis, min–max whiskers, speed multipliers vs the tsc
  baseline, colored diagnostic-drift badges) and peak memory (median bars on
  a MB/GB axis, whiskers, memory ratio vs tsc). The memory panel is omitted
  when no memory was sampled. Run metadata is in the header.
- `--html` renders a tabbed page — **Wall time** and **Peak memory** tabs —
  each with its chart panel and a per-project stats table
  (median/min/max/spread/runs/vs-tsc/drift, and memory/of-tsc/source on the
  memory tab), plus a run-metadata panel. Without memory data the page is a
  single untabbed timing view.

## Interpreting Results

- Benchmark results are local-machine-relative. SVG/HTML reports are visualization aids, not cross-machine marketing claims.
- JSON, SVG, and HTML labels include the Rust job count when one is recorded so mixed serial/parallel runs are not ambiguous.
- Compare wall-clock timings alongside the diagnostic drift status.
- This is not yet a claim of full compiler parity. See `REAL_PROJECT_COMPAT.md` for current compatibility limitations.

## Allocator Benchmark (`allocator-bench.ts`)

Compares the `surge` binary built with each supported global allocator:
`system` (default), `mimalloc`, `jemalloc`, `snmalloc`. Allocator selection is a
cargo feature of `surge-ts-cli` only (library crates never set a global
allocator); enabling more than one feature is a compile error.

```bash
pnpm run bench:allocators                       # build all 4 variants, run the full matrix
pnpm run bench:allocators -- --allocators system,mimalloc --iterations 5
pnpm run bench:allocators -- --scenario large --skipBuild   # reuse .bench/allocators/bin
```

What it does:

1. Builds each variant (`cargo build --release -p surge-ts-cli [--features <alloc>]`)
   and copies the binary to `.bench/allocators/bin/surge-<alloc>`.
2. Verifies each binary self-reports its allocator (`SURGE_PRINT_ALLOCATOR=1 surge`).
3. Runs a scenario matrix: small single-file cold run, medium project at
   `--jobs 1` and `--jobs auto` (`.local-projects/ky` when present, otherwise a
   generated synthetic fixture), large project at both job levels
   (`.local-projects/zod` or `trpc` when present), and a repeated-execution
   series to observe run-to-run peak-RSS stability.
4. Measures wall time and peak RSS per run with the same tested `/usr/bin/time`
   wrapper as `scripts/real-projects/measure-project.ts` (`-l` bytes on macOS,
   `-v` KiB on Linux; `unavailable` elsewhere). Final RSS of a short-lived
   process is not externally observable and is always reported `unavailable`.
5. Writes per-run records and per-scenario medians/worst peaks to
   `.bench/allocators/results.json` and `summary.md`.

Interpretation rules: compare median wall time and median/worst peak RSS over
at least 5 post-warmup runs, with `--jobs 1` and parallel results kept
separate. Do not change the default allocator without a repeatable ≥5% median
time win (no material RSS regression) or ≥10% peak-RSS win (no material time
regression) on real projects.
