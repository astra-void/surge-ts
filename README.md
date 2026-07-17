# surge-ts

`surge-ts` is a TypeScript type checker written in Rust, focused on
`tsc`-compatible diagnostics for `noEmit`-style project checking. It is not a
full TypeScript compiler and does not claim full TypeScript compatibility:
compatibility is measured, per feature and per project, against the upstream
compiler as an oracle. The engineering priorities are practical checking
performance, bounded memory use, deterministic output (identical diagnostics
across repeated runs and across serial/parallel execution), and validation on
real-world projects rather than synthetic claims.

The workspace ships an embeddable library (`surge-ts`, with the lower-level
`surge-ts-checker`) and a CLI (`surge-ts-cli`, binary name `surge`). The stable
API surface and the exact-parity feature list live in
[PUBLIC_API.md](PUBLIC_API.md).

## Quick start

Build the CLI (release profile — debug builds are much slower and not
representative):

```bash
cargo build --release -p surge-ts-cli
```

Check a project the way `tsc --noEmit` would:

```bash
target/release/surge --project path/to/tsconfig.json
```

```text
src/index.ts(11,7): error TS2322: Type 'string | undefined' is not assignable to type 'string'.
src/index.ts(19,12): error TS2339: Property 'missing' does not exist on type 'User'.
```

The exit code is `0` when there are no diagnostics and `2` when there are.
A single `.ts` file can also be passed positionally instead of
`--project`. Useful run-level flags: `--jobs <auto|N>` (worker threads; `auto`
is the default), `--maxDiagnostics <N>` (cap displayed diagnostics), and
`--pretty <true|false|auto>` (tsc-style code frames).

Machine-readable diagnostics on stdout:

```bash
target/release/surge --project path/to/tsconfig.json --format json
```

```json
{
  "diagnostics": [
    {
      "code": "TS2322",
      "fileName": "src/index.ts",
      "message": "Type 'string | undefined' is not assignable to type 'string'.",
      "span": { "start": 201, "end": 208 },
      "line": 11,
      "column": 7
    }
  ]
}
```

(Abbreviated to the first diagnostic; the array carries every diagnostic.)

Run statistics on stderr (diagnostics output on stdout is unchanged):

```bash
target/release/surge --project path/to/tsconfig.json --extendedDiagnostics
```

```text
Extended diagnostics:
  files:                                 64
    source files:                         1
    dependency declaration files:         0
    default lib files:                   63
  diagnostics:                            5
  jobs:                                auto
  allocator:                         system
  config/project loading:           0.398ms
  ...
  checking:                        18.902ms
  total:                           22.772ms
  peak physical footprint:         40.1 MiB
  finish physical footprint:       40.1 MiB
  peak rss:                        43.5 MiB
```

A versioned machine-readable run report (`schemaVersion: 1`), written to a
file:

```bash
target/release/surge --project path/to/tsconfig.json --reportJson report.json
```

```json
{
  "schemaVersion": 1,
  "summary": {
    "files": 64,
    "sourceFiles": 1,
    "dependencyDeclarationFiles": 0,
    "defaultLibFiles": 63,
    "diagnostics": 5,
    "wallTimeMs": 29.521,
    "jobs": "auto",
    "allocator": "system"
  },
  "phases": { "checkingMs": 23.965, "totalMs": 29.521 },
  "memory": {
    "peakPhysicalBytes": 43598280,
    "finishPhysicalBytes": 43598280,
    "peakRssBytes": 47153152
  }
}
```

(The `phases` object is abbreviated here; the full schema, including
`--memoryReport` and the guarantee that reporting flags never change the
diagnostics output, is documented in [PUBLIC_API.md](PUBLIC_API.md).)

## Current benchmark snapshot

One recorded workload, measured at commit `6fc9e6c` (2026-07-16) on an Apple
M1 Pro (10 cores, 16 GiB RAM, macOS 27.0), release build, system allocator:

| Workload | Metric | Value |
| --- | --- | ---: |
| tRPC repository (pinned checkout `3e0e9793eb7f`) | wall median, `--jobs 1` | 19.86 s |
| | wall median, `--jobs auto` | 19.70 s |
| | peak physical footprint | 3.75–3.88 GiB |
| | finish physical footprint | 1.96 GiB |

Caveats, all of which matter:

- This is **one specific project on one specific machine**, not a universal
  compiler comparison. Numbers do not transfer across projects, hardware,
  allocators, or build profiles.
- Runs are **cold-process over a warm filesystem cache**. There is no
  incremental or persistent mode; every run checks the full project.
- Exact reproduction requires the recorded fixture commit — the tRPC checkout
  is not distributed with this repository.

Full recorded-run details, methodology (including why interleaved A/B runs are
required for memory comparisons), and step-by-step reproduction instructions
are in [BENCHMARKS.md](BENCHMARKS.md).

## Correctness evidence

At commit `6fc9e6c`:

- **Workspace tests:** 1,521/1,521 passing (`cargo nextest run --workspace`),
  including gated compat-project fixtures.
- **Oracle preset sweep:** 83/83 registered presets green under the normal
  gate — diagnostic code-count and file/code/line match against the upstream
  TypeScript compiler (`pnpm run oracle:sweep -- --all --maxDiagnostics 200`).
  The preset count grows as fixtures are added.
- **Real-project regression suites:** projects where `tsc` reports zero
  diagnostics are pinned as strict false-positive corpora (e.g.
  `pnpm run real:ky:test`); any surge diagnostic on them is a regression.
- **Determinism:** repeated fresh runs render byte-identical diagnostics
  (pinned by in-process tests and by CLI stdout-hash checks in the complexity
  suite), and serial and parallel checking produce identical diagnostics —
  worker results merge in loaded-file order, never completion order. The
  recorded tRPC run pins one SHA-256 diagnostic hash across `--jobs 1` and
  `--jobs auto`.

What this evidence does and does not prove: the oracle gate establishes parity
with `tsc` **on the covered fixtures and projects**, and the regression suites
keep that parity from silently eroding. It does not establish full TypeScript
compatibility — projects outside the covered surface can and do produce
diagnostic drift, and known gaps are tracked openly in
[REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md). Message text and exact
span/column agreement are reported but non-gating unless the strict flags are
passed.

## Architecture overview

The workspace is split into small crates with stable facades: parsing
(`surge-ts-syntax`), the core type representation (`surge-ts-types`), semantic
checking (`surge-ts-checker`), diagnostics catalog/rendering
(`surge-ts-diagnostics`), tsconfig loading (`surge-ts-config`), the embeddable
umbrella crate (`surge-ts`), and the CLI (`surge-ts-cli`). Project checking is
a fixed phase pipeline — config load, file discovery, import-graph expansion
(including physical `lib*.d.ts` loading from the local `typescript` package),
parse, module analysis/binding, per-file checking (serial or parallel), and
rendering.

Two design decisions carry most of the performance and determinism weight.
First, each run owns a canonical `ProgramTypeStore` that interns structural
type payloads (function types, union member lists, property maps, parameter
lists) so identical types are shared write-once `Arc`s instead of repeated
allocations; type IDs embed a program-owner tag and never cross runs, and the
store is torn down at end of run so embedders do not retain the type graph.
Second, checker options and module-resolution tables are shared immutably for
the whole run (`Arc<CheckerOptions>`) — contexts are never deep-cloned per
module, and every cross-module cache must key on declaration identity,
arguments, and environment. The rules that keep this fast and deterministic
are written down as enforceable invariants: see
[ARCHITECTURE.md](ARCHITECTURE.md) and
[docs/PERFORMANCE_INVARIANTS.md](docs/PERFORMANCE_INVARIANTS.md).

## Compatibility status

Measured compatibility against real projects — what matches exactly, what
drifts, and the known false-positive taxonomies — is tracked in
[REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md). The verified-exact feature
list and the stable embedding API are in [PUBLIC_API.md](PUBLIC_API.md).

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
