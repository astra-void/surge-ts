# Current Status

**This document is the canonical description of what `surge-ts` is and does
right now.** Every other document in the repository is either a stable contract
([PUBLIC_API.md](PUBLIC_API.md)), a detailed measurement record
([REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md),
[BENCHMARKS.md](BENCHMARKS.md), [STRICT_DRIFT_INVENTORY.md](STRICT_DRIFT_INVENTORY.md)),
a design reference ([ARCHITECTURE.md](ARCHITECTURE.md)), or a point-in-time
engineering report ([docs/perf/](docs/perf/), [docs/history/](docs/history/)).
Where any of them disagrees with this file about *current* behavior, this file
wins and the other document should be corrected.

Volatile numbers (preset count, test count, real-project parity, benchmark
medians) are recorded **here only**. Other documents link to this file rather
than copying them.

---

## Verification snapshot

| Field | Value |
| --- | --- |
| Date | 2026-09-03 |
| Commit | `f63641d` (clean checkout; built and measured from a detached worktree at that commit) |
| Hardware | Apple M1 Pro (MacBookPro18,1), 10 cores, 16 GiB RAM |
| OS | macOS 27.0 (build 26A5425a) |
| Toolchain | rustc 1.94.0, Node v22.23.2, pnpm 11.13.0, TypeScript oracle 7.0.2 |
| Build profile | cargo `release` (`lto = "fat"`, `codegen-units = 1`), system allocator |

Commands run for this snapshot are listed in [§ How to re-verify](#how-to-re-verify).

**Is this snapshot current?** Compare the commit above with `git rev-parse
HEAD`. If they differ, the gate results below still describe the named commit
faithfully but may not describe `HEAD` — re-run the commands rather than
assuming they carry forward. Measure from a clean checkout: build the CLI in a
detached worktree at the commit you intend to cite and point the harness at it
with `SURGE_TS_BIN`, so an in-progress working tree cannot leak into a recorded
number.

**One number in this snapshot is deliberately absent.** Wall-clock benchmark
medians were **not** re-measured at this commit: the machine was carrying an
unrelated concurrent test run (load average ~120–145) for the whole
measurement window. Peak memory, diagnostic counts, and the output hash are
unaffected and are recorded; see
[§ Current performance state](#current-performance-state).

### Gates

| Gate | Command | Result |
| --- | --- | ---: |
| Workspace tests | `cargo nextest run --workspace` | **1764 / 1764 passed** |
| Oracle harness tests | `pnpm run oracle:test` | **23 / 23 passed** |
| Oracle preset sweep — normal gate | `pnpm run oracle:sweep -- --all --maxDiagnostics 200` | **119 / 119 passed** |
| Oracle preset sweep — `--strictMessages` | same + `--strictMessages` | 111 / 119 (8 message-text drifts) |
| Oracle preset sweep — `--strictSpans` | same + `--strictSpans` | 118 / 119 (1 span drift) |
| Oracle preset sweep — both strict flags | same + both | 110 / 119 (the two sets are disjoint) |
| Real-project gate — ky (exact 0/0) | `pnpm run real:ky:test` | **3 / 3 passed** |
| Real-project gate — unnamed (ceiling of 34) | `pnpm run real:unnamed:test` | **2 / 2 passed** — at 1 of 34 |

The normal gate compares **diagnostic code counts** and **file/code/line**
parity against the upstream TypeScript compiler. Across all 119 presets the
sweep saw 203 `tsc` diagnostics and 203 `surge-ts` diagnostics, with
`onlyTsc = 0` and `onlyRust = 0`. Message text and exact span/column are
**separate, non-gating dimensions** unless the strict flags are passed.

The 9 strict-drifting targets are **unchanged in membership** from the
2026-09-01 sweep and are inventoried, with their exact message and span deltas,
in [STRICT_DRIFT_INVENTORY.md § Current snapshot](STRICT_DRIFT_INVENTORY.md#current-snapshot-2026-09-01).
Only the header counts in that document (117 presets, 199 diagnostics) predate
this sweep; its drift tables still describe current behavior.

`diagnostics-pack`, the compact emitted-diagnostic fixture, is green at **31/31**
and passes the normal gate *and* both strict gates.

**No gate is red, but `unnamed` no longer holds the 0/0 it was recorded at on
2026-09-01.** Its enforced gate is a *count ceiling* of 34 over-reports, not an
exactness claim, so one surge-only diagnostic still passes; the exactness
recorded in the previous snapshot does not reproduce. The cause is diagnosed in
[§ Open over-report](#open-over-report-unnamed) below, and the ceiling is a
ratchet that should be lowered to 1 once a fixture pins the behavior.

---

## What surge-ts is

A TypeScript type checker written in Rust, aimed at `tsc`-compatible
diagnostics for `noEmit`-style project checking. It is **not** a TypeScript
compiler: there is no emit, no transform, no language service, no incremental
or watch mode.

Compatibility is *measured*, feature by feature and project by project, against
the upstream compiler used as an oracle. Nothing in this repository claims
"full TypeScript compatibility", and no targeted fixture result should ever be
generalized into one.

The workspace ships an embeddable library (`surge-ts`, with the lower-level
`surge-ts-checker`) and a CLI (`surge-ts-cli`, binary `surge`).

### Current scope

- Project mode driven by `tsconfig.json` (`extends` chains, `include`/`exclude`,
  compiler-option normalization), plus a narrow single-file mode.
- Physical `lib*.d.ts` loading from the local `typescript` package **by
  default**; the generated default-lib subset is only the fallback when that
  package is absent (and the single-file support path). `noLib: true` disables
  both.
- Declaration-side module resolution: relative imports, `paths`, `baseUrl`,
  package `exports` (conditional/pattern/subpath), package `imports`,
  `typesVersions`, package self-name, `types` / `typeRoots`, `@types`
  discovery under TS 6/7 semantics, and `/// <reference types>` directives
  followed recursively.
- Deterministic output: repeated runs render byte-identical diagnostics, and
  `--jobs 1` and `--jobs auto` produce identical diagnostics (worker results
  merge in loaded-file order, never completion order).

### Non-goals

- Emit, transforms, declaration emit, or a language service.
- Incremental checking, watch mode, or project references.
- Full runtime/JS package resolution (`main` entrypoints, wildcard `exports`
  runtime conditions, `rootDirs`).
- Full `lib.d.ts` / DOM / Node / React parity.
- Being a drop-in replacement for `tsc` on arbitrary projects.

---

## Verified exact parity

"Exact parity" means: held at diagnostic code-count and file/code/line parity
with `tsc` by a gate that fails CI on regression. The authoritative list of
verified-exact feature areas and the stable embedding API is
**[PUBLIC_API.md](PUBLIC_API.md)** — treat anything not listed there as
unstable or out of scope.

Summary of what backs it today:

- 119 oracle presets under `tests/compat-projects/`, all green at the normal
  gate (see the table above).
- 343 compat-project fixtures in total; the ones not registered as oracle
  presets are exercised by `cargo nextest run --workspace` instead.
- `diagnostics-pack` at exact 31/31, pinning duplicate-declaration
  (TS2451/TS2393), TDZ (TS2448 + TS2454), missing-return span placement
  (TS2355/TS2366) and use-site generic-arity spans (TS2314/TS2315).

---

## Real-project compatibility

Measured with `pnpm run oracle:compare -- --project <tsconfig> --maxDiagnostics
100000` at the snapshot commit against the pinned TypeScript 7.0.2 oracle.
Detailed history, drift taxonomies, and burn-down records live in
[REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md).

| Project | Checkout | `tsc` | `surge-ts` | Verdict |
| --- | --- | ---: | ---: | --- |
| **ky** (sindresorhus/ky 2.0.2) | `3419113` | 0 | 0 | **exact** — strict false-positive gate |
| **ofetch** (unjs/ofetch) | `1dbc37f` | 1 | 1 | **exact** — same file/code/line and message text (TS5108) |
| **zod** | `912f0f5` | 21 | 22 | 1 surge-only over-report (unchanged) |
| **unnamed** (local Next.js App Router app) | local | 0 | 1 | 1 surge-only over-report — **was 0/0 on 2026-09-01** |
| **trpc** | `dfbafa8` | 1244 | 1191 | measured baseline, **not** a parity target |
| **auth-kit** | — | — | — | **not measured** — the project is absent on this machine |

Notes that matter:

- Projects where `tsc` reports zero diagnostics (ky, unnamed) are
  false-positive regression corpora, but the two gates differ in strength.
  `pnpm run real:ky:test` asserts **exact 0/0** — any surge diagnostic fails it.
  `pnpm run real:unnamed:test` asserts a **count ceiling of 34** over-reports
  (a ratchet that only moves down) plus a precondition that `tsc` still reports
  0. Both *skip* cleanly when the project or the `typescript` package is absent
  (no third-party source is vendored).
- **zod** still over-reports by exactly one diagnostic:
  `packages/zod/src/v3/types.ts:92:42 TS2339 Property 'value' does not exist on
  type 'INVALID'.` All 21 `tsc` diagnostics are matched, message text included.
  zod is otherwise the most stable perf benchmark in the corpus.
- **trpc** is a *measured baseline*, never a parity claim. `tsc` itself reports
  over a thousand diagnostics there (many from examples with unresolved
  workspace imports), and the divergence is two-sided. At this commit the
  file/code/line drift is **207**: 130 diagnostics `tsc` reports and surge does
  not (led by TS7006 ×38, TS2339 ×29, TS2686 ×12, TS2883 ×10) and 77 surge-only
  (led by TS2339 ×18, TS7006 ×13, TS2322 ×10). On the 1,114 locations both
  compilers agree on, message text matches **1114 / 1114**.
- `auth-kit` is a private project that is not present on this machine. Its
  last recorded result was 0/0 (see
  [STRICT_DRIFT_INVENTORY.md § 11](STRICT_DRIFT_INVENTORY.md)); that figure is
  **not** carried forward as current, and this snapshot did not run it.

### Open over-report: `unnamed`

One surge-only diagnostic, in a Next.js data-table component:

```
app/[locale]/application/data-table.tsx:649:67
TS2322  Type 'undefined' is not assignable to type '() => void'.
```

**Root cause — a conditional expression checked against an optional property.**
When an object-literal (or JSX-attribute) value is a conditional expression, each
branch is checked against the *bare* declared type of the target property, with
the optionality-implied `undefined` stripped. The `undefined` branch then fails.
Minimal reproduction, on which `tsc` is silent:

```ts
interface Props { cb?: () => void; n?: number }
declare function take(p: Props): void;
declare const flag: boolean;

take({ cb: undefined });                  // ok
take({ cb: flag ? () => {} : undefined }); // surge-only TS2322
take({ n: flag ? 1 : undefined });         // surge-only TS2322

const maybe: (() => void) | undefined = flag ? () => {} : undefined;
take({ cb: maybe });                       // ok
```

The bug is **latent, not new**: the reproduction above fails identically on a
binary built at `37dfb3a`, the previous snapshot commit, where `unnamed` was
nevertheless 0/0. What changed is reachability — the offending attribute sits
inside a `return (…)` with no contextual type, and `10a1f5e` ("check a return
expression that has no expected type") is what began checking that position.
Bisected by building each commit in a clean worktree and re-running the same
comparison:

| Commit | `tsc` | `surge-ts` |
| --- | ---: | ---: |
| `37dfb3a` (previous snapshot) | 0 | 0 |
| `27c0b92` (parent of `10a1f5e`) | 0 | 0 |
| `10a1f5e` (start of the 2026-09-03 work) | 0 | **3** |
| `f63641d` (this snapshot) | 0 | 1 |

`10a1f5e` introduced all three; two of them (`TS2741`, a missing `href` on a
`Link` in two email templates) were closed by the 2026-09-03 commits. This one
was not, and no fixture pins it yet.

---

## Known limitations

Each entry below carries the date it was last reproduced or refuted against the
oracle. Entries marked **2026-09-03** were re-probed at `f63641d` for this
snapshot; entries marked **2026-09-01** are carried forward from the previous
snapshot without re-verification and are labelled as such.

### Confirmed current gaps

- **Overload resolution uses the first overload only.** *(re-probed
  2026-09-03.)* A call that should select a later overload resolves against the
  first signature. Today this surfaces as an *under-report*:
  `declare function f(a: string): string; declare function f(a: number):
  number;` then `const bad: string = f(1)` — `tsc` reports TS2322, surge
  reports nothing. A full overload-resolution implementation exists on a branch
  but is blocked on a measured ~+79% CPU regression.
- **A conditional expression assigned to an optional property drops the
  implied `undefined`.** *(found 2026-09-03.)* See
  [§ Open over-report](#open-over-report-unnamed) — the only known open false
  positive on a corpus where `tsc` reports nothing.
- **Module augmentation is lost through a star re-export wrapper.**
  *(re-probed 2026-09-03.)* With `declare module "core"` in a `.d.ts` and
  `export * from "core"` in `wrapper`, importing the augmented interface from
  `wrapper` yields the unaugmented shape (a surge-only excess/unknown-property
  error), while importing from `core` directly is correct.
- **`typeof import("pkg")` in a type alias does not resolve.** *(re-probed
  2026-09-03.)* `type T = typeof import("./dep")` leaves `T` unbound: TS2304 at
  the alias use site. Dynamic `import()` and `import("m").T` are parser gaps as
  well.
- **Generic JSX components: type-parameter-dependent prop mismatches are
  missed.** *(2026-09-01, not re-probed at this commit.)*
  `<List<string> items={[1]} />` is accepted; non-generic props on generic
  components *are* checked. Explicit-type-argument and value-inference paths in
  the registered React fixtures are correct.
- **Project references, incremental checking, and watch mode are not
  implemented.** *(re-probed 2026-09-03.)* There is no `--build`, `--watch`, or
  `--incremental` flag; every run is a full project check.
- **Full runtime/JS package resolution parity** (`main` entrypoints, wildcard
  `exports` runtime conditions, `rootDirs`, `preserveSymlinks`,
  `forceConsistentCasingInFileNames`) remains out of scope. The declaration
  side is the supported surface. The per-rule inventory, including the
  intentional `paths`-match-authority divergence, is in
  [crates/surge-ts/MODULE_RESOLUTION.md](crates/surge-ts/MODULE_RESOLUTION.md).
- **Full `lib.d.ts` / DOM / Node / React parity** remains out of scope even
  though the physical lib graph loads by default.

### Refuted on 2026-09-03

- **"Unresolved-module diagnostic policy differs from `tsc` by design."** The
  previous snapshot recorded that surge binds an unresolved module to its
  degradation sentinel and therefore *suppresses* implicit-any reports that
  `tsc` emits. That does not reproduce. On an import from a missing package,
  surge matches `tsc` exactly — same codes, same lines, same columns — on a
  plain implicit-any parameter (TS7006), on the two destructured binding
  elements of a callback passed to a value imported from the missing module
  (TS7031), and on two more in an unrelated untyped destructuring. The same
  probes match on a `37dfb3a` binary, so the entry was already stale when it
  was written.

  What remains true is narrower and is *not* the same claim: trpc still
  under-reports 130 diagnostics that `tsc` emits, TS7006 being the largest
  class. That gap has not been traced to a single policy, and the three
  broadenings tried on 2026-09-03 (binding unresolved type imports to `Any`,
  treating any `Type::Any` receiver as genuine, and early module-local value
  binding) each cost three to four times more false positives than they
  recovered. Do not restate the sentinel-suppression claim without a
  reproduction.

### Previously documented, now fixed (verified 2026-09-01)

These were listed as limitations in older documents and no longer reproduce.
They are recorded here so the old lists are not read as current. They were not
re-probed at `f63641d`.

- **Qualified heritage clauses** (`interface X extends NS.Member`,
  `class C extends NS.Base`) now resolve. The
  `tests/compat-projects/interface-qualified-heritage-basic` fixture measures
  **8/8 exact parity, message text included**. It is still *not registered* as
  an oracle preset — registering it is open follow-up work, not a blocker.
- **Reopened namespace/interface merging is now on by default**
  (opt-*out* via `SURGE_NS_IFACE_MERGE=0`), together with the dotted-name
  retry (`SURGE_NS_QUALIFIED_RETRY`). The older documentation describing it as
  gated off behind `SURGE_NS_IFACE_MERGE=1` is historical.
- **Generic inference from a callback's return type** (`fn: () => T` and
  `fn: () => Promise<T>`) now infers and reports like `tsc`.
- **Conditional `infer` inside a mapped-type body or an object-literal type
  position** (`{ initial: infer V }`) no longer produces a false TS2304.
- **`async function f(): Promise<void> {}` with no return statement** no longer
  raises a false TS2355.
- **A required `string | undefined` property against an optional target
  property** inside generic comparisons no longer produces a surge-only
  TS2322.
- **Transitive `/// <reference types="…" />` loading from dependency
  declaration files** is implemented, which closed the ofetch `node:*` gap.
- **Automatic `@types` discovery** is implemented to TS 6/7 semantics —
  including the fact that TypeScript 6.0+ does *not* implicitly include every
  visible `@types` package, and that `types: ["*"]` is the oracle-faithful
  wildcard. See [crates/surge-ts-cli/AUTO_TYPES.md](crates/surge-ts-cli/AUTO_TYPES.md).
- **`baseUrl` non-relative specifier resolution** is supported in the loader
  (the option is deprecated upstream but honored for compatibility), as are
  `export =` / `import … = require(…)`, enums, and namespaces.

---

## Experimental and feature-gated behavior

Runtime behavior gates use the `SURGE_` environment-variable prefix. Nothing
here is part of the stable surface; gates may be removed once their default is
settled.

**Opt-in (off by default) — changes checking behavior or performance:**

| Gate | Effect |
| --- | --- |
| `SURGE_AMBIENT_BLOCK_IMPORTS=1` | Bind imports inside ambient `declare module` blocks. Diagnostics set-identical on the corpus, but costly (~+36% user time on trpc) until member-level lazy expansion lands. |
| `SURGE_LAZY_IFACE_MEMBERS=1` | Stage 1 of member-level lazy interface expansion (property annotations only; methods stay eager). Design: [docs/perf/MEMBER-LAZY-EXPANSION.md](docs/perf/MEMBER-LAZY-EXPANSION.md). |
| `SURGE_LIB_MEMBER_CACHE=1` | Extended per-member instantiation cache. Measured at roughly +2% and left off. |
| `SURGE_IFACE_CACHE_ALL=1` | Extended interface-instantiation cache. Measured at +1.7% and left off. |

**Default-on with an opt-out kill switch** (for A/B profiling and regression
isolation only — production behavior is the default): `SURGE_NS_IFACE_MERGE`,
`SURGE_NS_QUALIFIED_RETRY`, `SURGE_LAZY_DTS_VALUES`, `SURGE_THIN_PRELIM`,
`SURGE_MODULE_TYPE_DEDUP`, `SURGE_EAGER_DEPENDENCY_ALIASES`,
`SURGE_DISABLE_CANONICAL_*`, `SURGE_DISABLE_SIG_CONTEXT_CACHE`,
`SURGE_DISABLE_PHYSICAL_INTERFACE_*`.

**Instrumentation only** (no semantic effect): `SURGE_TIMINGS`, `SURGE_RSS`,
`SURGE_RETENTION_CENSUS`, `SURGE_PAUSE_AT_STAGE`, `SURGE_ALLOCATION_CENSUS`,
`SURGE_TYPE_GRAPH_CENSUS`, and the various `SURGE_TRACE_*` / `SURGE_*_STATS`
gates. The hidden CLI flags `--timings` and `--rss` set the first two.

---

## Current performance state

One workload, one machine, one commit. **These numbers do not transfer** across
projects, hardware, allocators, or build profiles, and they are not a compiler
comparison.

tRPC monorepo at `.local-projects/trpc`, checkout `dfbafa8` (2026-07-26),
project mode, `--jobs auto`, cold process over a warm filesystem cache, command
`surge --project .local-projects/trpc/tsconfig.json --format json --maxDiagnostics 10000 --jobs auto`.
Measured as an **interleaved A/B**: two release binaries built from clean
worktrees at `37dfb3a` (the previous snapshot) and `f63641d` (this one), run
alternately, ten runs each after one warmup per binary.

| Metric | `37dfb3a` | `f63641d` |
| --- | ---: | ---: |
| Peak physical footprint | 1.852–1.861 GB | **1.040–1.053 GB** |
| Diagnostics emitted | 1,228 | **1,191** |
| Diagnostic output SHA-256 | `b49dc940…5ceb11b1` | `af8e047d…a5561474` |
| Wall time, median — **invalid, see caveat** | 13.4 s | 7.9 s |

**Peak memory is the reliable result here: a 44% reduction, with each binary's
ten runs spreading by under 1.5%.** It also cross-validates against the
previous snapshot, which independently recorded 1.856–1.863 GB for the
`37dfb3a` binary on 2026-09-01.

The diagnostic hash is byte-identical across all ten runs of each binary, and a
separate `--jobs 1` run of `f63641d` produces that same
`af8e047d…a5561474` — that is the determinism evidence, and it is a stronger
artifact than any wall-clock median. The `37dfb3a` hash reproduces the value
recorded in the 2026-09-01 snapshot exactly, which confirms the two snapshots
measured the same workload.

**Wall time was not measured under acceptable conditions and should not be
quoted as a snapshot number.** The machine carried an unrelated concurrent test
run (load average 120–145) throughout the window. The evidence that this
invalidates the absolute figures is direct: the *same* `37dfb3a` binary medians
at 13.4 s today against the 9.11 s recorded for it on 2026-09-01. Only the
interleaved ratio survives that, and even it is noisy — per-pair ratios ranged
0.96×–2.23× over ten pairs (one pair had `f63641d` slower), with a median near
1.7×. Re-measure on a quiet machine before recording a median here or in
[BENCHMARKS.md](BENCHMARKS.md).

**Other caveats, all of which still matter:**

- The local tRPC checkout is `dfbafa8`, **not** the `3e0e979` commit pinned by
  the historical run in [BENCHMARKS.md](BENCHMARKS.md). The workload itself
  differs, so neither column above is comparable to the older 19.7–19.9 s
  figures; treat that row as a separate measurement, not a before/after pair.
- Peak RSS on this workload varies ±30–50% run to run *in general*; the tight
  spread observed here is a property of this measurement session, not a
  guarantee. Memory comparisons require interleaved A/B runs in one session,
  never two batches measured at different times. See
  [BENCHMARKS.md § Methodology](BENCHMARKS.md#methodology).
- There is no incremental or persistent mode; every run is a full check.
- A speed number is only meaningful alongside a known diagnostic surface. The
  surface changed between these two commits (1,228 → 1,191 diagnostics, with
  trpc file/code/line drift at 207 on the newer one), so this is **not** a
  like-for-like speed comparison of identical work.

Performance history, methodology, and the reproduction recipe live in
[BENCHMARKS.md](BENCHMARKS.md); the detailed engineering investigations live in
[docs/perf/](docs/perf/) and are point-in-time records, not current state.

---

## What is deliberately not claimed

- **No full TypeScript compatibility claim.** The oracle gate establishes
  parity on the covered fixtures and projects only.
- **No message-text or span/column parity claim** beyond what the strict
  sweeps actually show (currently 8 message drifts and 1 span drift across 119
  presets, all of which still match code-count and file/code/line).
- **No claim that `unnamed` is at exact parity.** It was 0/0 on 2026-09-01 and
  is 0/1 here; its enforced gate is a ceiling, not an exactness assertion.
- **No claim that trpc matches `tsc`.** It is a workload and a measured
  baseline.
- **No wall-clock performance claim at this commit** — see the caveat above.
- **No cross-tool performance claim.** `pnpm bench:compilers` exists as a
  developer aid; its output is local-machine-relative and is not a marketing
  comparison.
- **No claim about `auth-kit` at this commit** — the project is absent here and
  the 0/0 figure is a prior recorded result.
- **No suppression-transparency claim yet.** ky's source-level parity is 0/0,
  but three non-zero suppression counters (`suppressedRustOnly`,
  `suppressedDeclaration`, `externalModuleStubs`) remain pending an audit
  tracked in
  [crates/surge-ts-checker/SUPPRESSED_DIAGNOSTICS_AUDIT.md](crates/surge-ts-checker/SUPPRESSED_DIAGNOSTICS_AUDIT.md).

---

## Versioning

Three different version namespaces appear in this repository, and they are
**intentionally unrelated**:

| Namespace | Value | Meaning |
| --- | --- | --- |
| Cargo workspace version | `0.1.0` (`Cargo.toml` `[workspace.package]`, inherited by every crate) | The crate version. It is the only version a consumer sees. |
| npm workspace version | `0.0.0` (`package.json`) | Placeholder for the private dev-tooling workspace; nothing is published from it. |
| `v0.x` / `v1.x` labels in prose | e.g. `v0.85`, `v1.2.5` | **Internal milestone labels** used in historical engineering notes to date a change. They are not releases, not tags, and not crate versions. |

Do not synchronize these. A `v1.2.5` note in `ARCHITECTURE.md` or
`REAL_PROJECT_COMPAT.md` describes *when* something happened in the project's
own milestone numbering, not what version of anything you can install.

---

## Where to look next

| Question | Document |
| --- | --- |
| What API can I embed against? What is held at exact parity? | [PUBLIC_API.md](PUBLIC_API.md) |
| How does the checker work internally? | [ARCHITECTURE.md](ARCHITECTURE.md) |
| What are the detailed real-project measurements and their history? | [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md) |
| What message/span drift remains, and why? | [STRICT_DRIFT_INVENTORY.md](STRICT_DRIFT_INVENTORY.md) |
| How were the benchmarks taken, and how do I reproduce them? | [BENCHMARKS.md](BENCHMARKS.md) |
| What rules must performance-sensitive changes obey? | [docs/PERFORMANCE_INVARIANTS.md](docs/PERFORMANCE_INVARIANTS.md) |
| How is retained memory governed? | [crates/surge-ts-checker/MEMORY_REGIONS.md](crates/surge-ts-checker/MEMORY_REGIONS.md), [docs/MEMORY-OPTIMIZATION-REPORT.md](docs/MEMORY-OPTIMIZATION-REPORT.md) |
| How exactly does module resolution behave? | [crates/surge-ts/MODULE_RESOLUTION.md](crates/surge-ts/MODULE_RESOLUTION.md) |
| How does `@types` / `typeRoots` discovery work? | [crates/surge-ts-cli/AUTO_TYPES.md](crates/surge-ts-cli/AUTO_TYPES.md) |
| Why is the program checked in two analysis rounds? | [crates/surge-ts-checker/PROGRAM_CHECKING.md](crates/surge-ts-checker/PROGRAM_CHECKING.md) |
| How did a particular optimization land (or get rejected)? | [docs/perf/](docs/perf/) — point-in-time reports |
| What did the project look like earlier? | [docs/history/](docs/history/) |

---

## How to re-verify

Everything in the snapshot above comes from these commands, run against a clean
checkout of the snapshot commit:

```bash
cargo nextest run --workspace
```

```bash
pnpm run oracle:test
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictSpans
```

```bash
pnpm run oracle:sweep -- --all --maxDiagnostics 200 --strictMessages --strictSpans
```

```bash
pnpm run oracle:compare -- --project .local-projects/ky/tsconfig.json --maxDiagnostics 100000
```

```bash
pnpm run real:ky:test && pnpm run real:unnamed:test
```

The strict sweeps exit non-zero by design when drift exists; that is the
expected outcome, not a failure of the run. `--strictMessages --strictSpans`
must be passed as two separate arguments; the sweep rejects them quoted as one.

Real-project targets are gated on the checkout being present locally
(`.local-projects/` is gitignored and no third-party source is vendored). When
a project is absent, record it as *not measured* rather than carrying forward
an older number as current.

**Rule of thumb when updating this file:** do not change a number here unless
you ran the command that produces it. If you are recording someone else's
result, say so explicitly and cite the source document, commit, and date. If
you could not measure a number under valid conditions, say that — an absent
number is honest, a number measured on a loaded machine is not.
