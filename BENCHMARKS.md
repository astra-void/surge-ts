# Benchmarks

Recorded performance measurements for `surge-ts`, with enough context to
reproduce them. Every number here is tied to one workload, one machine, and
one commit — none of it is a universal compiler comparison. Comparisons
against other tools or other configurations must be re-measured on the same
machine in the same session (see Methodology).

## Recorded run: tRPC repository

| Field | Value |
| --- | --- |
| Date | 2026-07-16 |
| surge-ts commit | `6fc9e6c` |
| Fixture | tRPC repository checkout, commit `3e0e9793eb7f8c4cfbe70a1dccb72f8d355e3c8b`, placed at `.local-projects/trpc` |
| Hardware | Apple M1 Pro, 10 cores, 16 GiB RAM |
| OS | macOS 27.0 (build 26A5378n) |
| Toolchain | rustc 1.94.0, Node v24.12.0, pnpm 11.13.0 |
| Build profile | cargo `release` |
| Allocator | system (the default; see allocator notes below) |
| Command | release `surge --project .local-projects/trpc/tsconfig.json` (project mode), cold process per run |
| Run policy | warm filesystem cache, median of 3 cold-process runs |

Results:

| Metric | jobs = 1 | jobs = auto |
| --- | ---: | ---: |
| Wall time, median | 19.86 s | 19.70 s |
| Wall time, min / max | 19.57 s / 20.39 s | median only recorded |
| Peak physical footprint (`phys_footprint`) | 3.75–3.88 GiB across runs | same range |
| Finish physical footprint | 1.96 GiB | — |
| `FunctionType` payloads created | 942,756 | — |
| Diagnostic output SHA-256 | `4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59` | identical |

The diagnostic hash pins the complete diagnostic output: it is identical
between `--jobs 1` and `--jobs auto`, and identical across repeated runs. A
per-run diagnostic count is intentionally not published here because no
validated count was recorded alongside this run; the hash is the stronger
artifact.

Correctness gates at the same commit: workspace tests 1,521/1,521
(`cargo nextest run --workspace`) and oracle preset sweep 83/83
(`pnpm run oracle:sweep -- --all --maxDiagnostics 200`).

## Methodology

- **Cold process, warm filesystem cache.** Each run is a fresh `surge`
  process; the OS file cache is warm from previous runs. There is no
  incremental or persistent mode in surge-ts — every run is a full project
  check, so these numbers are full-check numbers.
- **Median of at least 3 runs.** Wall time is reported as the median of 3+
  cold-process runs (the benchmark harness defaults to 5 iterations plus 1
  warmup per configuration). Single runs are never published.
- **Interleaved A/B runs for memory comparisons.** Peak-RSS on this workload
  varies ±30–50% run-to-run. Comparing a "before" batch against an "after"
  batch measured at different times is meaningless at that noise level;
  alternate A/B/A/B in one session and compare medians. The
  `phys_footprint`-based peak (reported by `--extendedDiagnostics`,
  `--memoryReport`, and `--reportJson`) is the Activity-Monitor-comparable
  figure on macOS; deterministic instrumentation counters (`--timings`, and
  the payload counts above) are the stable cross-run signal.
- **Same binary, same allocator, same profile.** Wall time and memory from
  different allocators, build profiles, or binaries must not be naively
  compared — allocator choice alone shifts both. The recorded run uses the
  default system allocator; `pnpm bench:allocators` exists precisely to make
  allocator comparisons controlled.
- **Diagnostics are part of the measurement.** A speed number is only valid
  alongside an unchanged diagnostic surface; runs are paired with the
  diagnostic hash above and with oracle drift checks.

## Reproduction

1. Clone this repository and install the JS tooling:

   ```bash
   pnpm install
   ```

2. Place a tRPC checkout at `.local-projects/trpc`, pinned to the recorded
   commit (the checkout is not distributed with this repository):

   ```bash
   git clone https://github.com/trpc/trpc .local-projects/trpc
   git -C .local-projects/trpc checkout 3e0e9793eb7f8c4cfbe70a1dccb72f8d355e3c8b
   ```

3. Build the release CLI:

   ```bash
   cargo build --release -p surge-ts-cli
   ```

4. Run the measurement harness:

   ```bash
   pnpm real:trpc
   ```

   This runs `scripts/real-projects/measure-project.ts --project
   .local-projects/trpc --name trpc --allowMissing`, which: compares
   diagnostics against the TypeScript oracle (`oracle:compare`, text + JSON),
   rebuilds the release binary, samples peak RSS per output mode under
   `/usr/bin/time`, runs the compiler benchmark at each requested job level
   (default `--rustJobs 1,auto`; 5 iterations + 1 warmup each), and writes all
   artifacts plus a `measurement.md` summary under
   `.bench/real-projects/trpc/`. Useful options: `--maxDiagnostics <N>`
   (default 500), `--rustJobs <list>`, `--outDir <dir>`; `--allowMissing`
   makes a missing checkout a recorded no-op instead of a failure.

   For a single manual timing run without the harness:

   ```bash
   target/release/surge --project .local-projects/trpc/tsconfig.json --jobs 1 --extendedDiagnostics
   ```

Expect different absolute numbers on different hardware, OS versions, or
toolchains; the fixture commit pin is what makes the workload itself
comparable.

## Related harnesses

- `pnpm bench:compilers` — wall-clock comparison of `tsc` (TS 6), `tsgo`
  (native TS 7), and the release `surge` binary over the same tsconfig, with
  diagnostic-drift reporting.
- `pnpm bench:allocators` — builds the `surge` binary with each supported
  global allocator (system, mimalloc, jemalloc, snmalloc) and runs a
  wall-time/peak-RSS scenario matrix.
- `pnpm bench:archive` — wraps benchmark and real-project runs and archives
  their raw output plus a summary under `.bench/runs/<timestamp>/` for
  before/after comparison.
- `pnpm bench:complexity` — deterministic complexity-scaling regression
  suite: generates synthetic projects at multiple sizes and gates on
  instrumentation-counter growth (constant/linear/superlinear), not wall
  time.

Details for all four live in [scripts/bench/README.md](scripts/bench/README.md).

## Engineering history (same workload, same machine)

Both rows below are the tRPC workload above on the same machine, release
build, system allocator. Earlier optimization stages exist but their exact
configurations were not recorded well enough to publish.

| Stage | Wall median (jobs = 1) | Peak physical footprint |
| --- | ---: | ---: |
| Canonical-type-graph stage | 47.42 s | ~3.86 GiB |
| CPU-optimization pass (commit `6fc9e6c`) | 19.86 s | 3.75–3.88 GiB |

Measurements taken from different binaries, allocators, or build profiles are
not comparable to this table without re-running both sides interleaved on the
same machine.
