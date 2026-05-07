# Compiler Speed Benchmark Harness

This directory contains the developer-facing regression benchmark (`compare-compilers.ts`) for measuring wall-clock performance and diagnostic drift.

It is **not** a marketing benchmark. It compares the same `tsconfig.json` inputs across:

1. JavaScript TypeScript compiler (`tsc`, pinned through workspace dependency)
2. TypeScript native Go compiler (`tsgo`, optional)
3. `typescript-rust` CLI (using the release binary, not `cargo run`)

## Requirements

1. **Release Binary Required:** The benchmark tests the prebuilt release binary, not debug builds. Build it first:

    ```bash
    cargo build --release -p typescript-rust-cli
    ```

2. **`tsgo` is Included When Available:** `@typescript/native-preview` does not need to be installed globally. If `tsgo` is present in the workspace, the harness will benchmark it automatically and still cleanly skip it when it is unavailable.
3. **TS 7-Oriented Fixtures:** Committed fixtures under `tests/compat-projects/` must not use `ignoreDeprecations` in their `compilerOptions`. If a fixture triggers a TS 6 deprecation warning, fix the fixture config instead of suppressing the diagnostic. The harness actively guards against `ignoreDeprecations` usage.

## What It Measures

The benchmark compares **no-emit project checking** only:

- `tsc --noEmit --pretty false --project <tsconfig>`
- `tsgo --noEmit --pretty false --project <tsconfig>`
- `typescript-rust-cli --project <tsconfig> --format json --maxDiagnostics 10000`
- `typescript-rust-cli --project <tsconfig> --format json --maxDiagnostics 10000 --jobs <n>` for deterministic project-checking measurements with explicit Rust worker counts

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

Change iterations, generate visual reports, and output JSON:

```bash
pnpm run bench:compilers -- --preset current --iterations 10 --warmup 2 --json .bench/compiler-bench.json --chart .bench/compiler-bench.svg --html .bench/compiler-bench.html
```

Measure Rust at a specific worker count:

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

## Running Benchmark Tests

To verify the harness itself:

```bash
cargo build --release -p typescript-rust-cli
pnpm run bench:test
```

## Interpreting Results

- Benchmark results are local-machine-relative. SVG/HTML reports are visualization aids, not cross-machine marketing claims.
- JSON, SVG, and HTML labels include the Rust job count when one is recorded so mixed serial/parallel runs are not ambiguous.
- Compare wall-clock timings alongside the diagnostic drift status.
- This is not yet a claim of full compiler parity. See `REAL_PROJECT_COMPAT.md` for current compatibility limitations.
