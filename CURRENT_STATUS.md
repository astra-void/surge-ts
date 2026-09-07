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
| Date | 2026-09-07 |
| Commit | `bda16e0` (clean checkout; built and measured from a detached worktree at that commit) |
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

**This snapshot carries an open regression that is not from the work at this
commit.** tRPC's peak memory footprint and wall time roughly doubled between the
previous snapshot and this one, bisected to `4b3bcc0`; the measurement and the
bisect are in [§ Current performance state](#current-performance-state).

### Gates

| Gate | Command | Result |
| --- | --- | ---: |
| Workspace tests | `cargo nextest run --workspace` | **1828 / 1828 passed** |
| Oracle harness tests | `pnpm run oracle:test` | **23 / 23 passed** |
| Oracle preset sweep — normal gate | `pnpm run oracle:sweep -- --all --maxDiagnostics 200` | **136 / 136 passed** |
| Oracle preset sweep — `--strictMessages` | same + `--strictMessages` | **136 / 136 passed** |
| Oracle preset sweep — `--strictSpans` | same + `--strictSpans` | **136 / 136 passed** |
| Oracle preset sweep — both strict flags | same + both | **136 / 136 passed** |
| Real-project gate — ky (exact 0/0) | `pnpm run real:ky:test` | **3 / 3 passed** |
| Real-project gate — unnamed (ceiling of 34) | `pnpm run real:unnamed:test` | **2 / 2 passed** — at 0 of 34 |

The normal gate compares **diagnostic code counts** and **file/code/line**
parity against the upstream TypeScript compiler. Across all 136 presets the
sweep saw 225 `tsc` diagnostics and 225 `surge-ts` diagnostics, with
`onlyTsc = 0` and `onlyRust = 0`.

**Message text and span/column match on every registered preset.** Both strict
sweeps have been green since 2026-09-03; the nine drifting targets recorded on
2026-09-01 were closed then, and their deltas plus the fixes are kept in
[STRICT_DRIFT_INVENTORY.md § Current snapshot](STRICT_DRIFT_INVENTORY.md#current-snapshot-2026-09-03).
The strict flags are still *separate dimensions* — a future preset may reopen
one without failing the normal gate — so the exit codes remain non-gating in CI
even though they currently exit zero.

`diagnostics-pack`, the compact emitted-diagnostic fixture, is green at **31/31**
and passes the normal gate *and* both strict gates.

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

- 136 oracle presets under `tests/compat-projects/`, all green at the normal
  gate **and at both strict gates** (see the table above).
- 360 compat-project fixtures in total; the ones not registered as oracle
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
| **zod** | `912f0f5` | 21 | 21 | **exact** — every diagnostic matched, message text included |
| **unnamed** (local Next.js App Router app) | local | 0 | 0 | **exact** — strict false-positive corpus |
| **trpc** | `dfbafa8` | 1244 | 1153 | measured baseline, **not** a parity target |
| **auth-kit** | — | — | — | **not measured** — the project is absent on this machine |

Notes that matter:

- Projects where `tsc` reports zero diagnostics (ky, unnamed) are
  false-positive regression corpora, but the two gates differ in strength.
  `pnpm run real:ky:test` asserts **exact 0/0** — any surge diagnostic fails it.
  `pnpm run real:unnamed:test` asserts a **count ceiling of 34** over-reports
  (a ratchet that only moves down) plus a precondition that `tsc` still reports
  0. Both *skip* cleanly when the project or the `typescript` package is absent
  (no third-party source is vendored).
- **zod is exact at this commit.** All 21 `tsc` diagnostics are matched at
  file/code/line *and* message text (21/21). The one long-standing over-report
  (`packages/zod/src/v3/types.ts:92:42 TS2339`) was a generic type-predicate
  guard that never narrowed; it is closed. zod remains the most stable perf
  benchmark in the corpus.
- **trpc** is a *measured baseline*, never a parity claim. `tsc` itself reports
  over a thousand diagnostics there (many from examples with unresolved
  workspace imports), and the divergence is two-sided. At this commit the
  file/code/line drift is **155**: 123 diagnostics `tsc` reports and surge does
  not (led by TS7006 ×38, TS2339 ×29, TS2883 ×10, TS18048 ×8) and 32 surge-only
  (led by TS2339 ×9, TS2349 ×5, TS2322 ×4, TS7006 ×3, TS4111 ×3). On the 1,121
  locations both compilers agree on, message text matches **1121 / 1121**.
  The surge-only side came down from 65 at `019fb8b` over the 2026-09-07
  session; what remains is listed in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-surge-only-inventory-2026-09-07).
- `auth-kit` is a private project that is not present on this machine. Its
  last recorded result was 0/0 (see
  [STRICT_DRIFT_INVENTORY.md § 11](STRICT_DRIFT_INVENTORY.md)); that figure is
  **not** carried forward as current, and this snapshot did not run it.

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

### Fixed at this commit (2026-09-03)

- **Every strict-sweep drift.** The nine targets recorded on 2026-09-01 — eight
  message-text, one span — are closed; both strict sweeps are green. The fixes
  and the rule that made them safe (display metadata rides on the type *handle*
  or the diagnostic layer, never the interned payload) are in
  [STRICT_DRIFT_INVENTORY.md](STRICT_DRIFT_INVENTORY.md).
- **A conditional assigned to an optional property** no longer reports a
  surge-only TS2322 on its `undefined` branch, which restored `unnamed` to 0/0.
- **A generic user-defined type predicate now narrows** (`x is OK<T>` with `T`
  inferred from the tested argument), which took zod to exact 21/21.
- **`void` / `delete` / `~` operands are checked.** They previously lowered to a
  node that dropped the operand, so nothing inside them was ever seen.

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
project mode, `--jobs auto`, cold process over a warm filesystem cache, peak
*physical footprint* from `/usr/bin/time -l`. Every column is an **interleaved
A/B** against release binaries built from clean worktrees at the named commits.

| Metric | `b090760` (prev. snapshot) | `019fb8b` (session start) | `bda16e0` (this commit) |
| --- | ---: | ---: | ---: |
| Peak physical footprint, median | 1.013 GB | 2.012 GB | **2.009 GB** |
| Wall time, median | 4.85 s | 8.23 s | **8.06 s** |
| Diagnostics emitted | 1,190 | 1,186 | **1,153** |

`b090760` ↔ `bda16e0` is four interleaved pairs and `b090760` ↔ `019fb8b` three,
both on a quiet machine (load average ~6). `019fb8b` ↔ `bda16e0` is twelve
pairs, taken earlier under an unrelated external workload (load averages
72–106): the footprint held at 2.004–2.017 GB on both sides and the wall-time
medians straddled zero (8.73 s vs 8.97 s), so **this session is neutral on
both** and its wall-time delta is not quotable more tightly than that.

**There is an open ~2x memory regression on this workload, and it is not from
this session.** Peak footprint doubled and wall time roughly doubled somewhere
between the previous snapshot and the start of the 2026-09-07 session. Bisected
by interleaved measurement over that range:

| Commit | Peak footprint | Wall |
| --- | ---: | ---: |
| `b08adb7` fix(check): narrow Array.filter by a type-predicate callback | 1.011 GB | 5.7–7.0 s |
| `4b3bcc0` fix(program): merge declare-global types before lowering ambient script values | **2.009 GB** | 10.8 s |

Those two are adjacent, so `4b3bcc0` is the commit that introduced it. Later
commits recovered part of the wall-clock cost (10.8 s → ~8 s) but none of the
memory. This is unresolved and unattributed work, recorded here rather than
carried silently: [AGENTS.md](AGENTS.md) makes a doubling on the flagship
workload a blocker-class number.

The diagnostic count moved because that was the point of the 2026-09-07 session:
33 fewer emitted diagnostics on tRPC, every one of them a surge-only
over-report (see [§ Real-project compatibility](#real-project-compatibility)).

**Caveats, all of which matter:**

- The local tRPC checkout is `dfbafa8`, **not** the `3e0e979` commit pinned by
  the historical run in [BENCHMARKS.md](BENCHMARKS.md). The workload differs, so
  neither column is comparable to the older 19.7–19.9 s figures.
- Peak RSS on this workload varies ±30–50% run to run *in general*; the tight
  spread here is a property of interleaving, not a guarantee. Memory comparisons
  require interleaved A/B runs in one session. See
  [BENCHMARKS.md § Methodology](BENCHMARKS.md#methodology).
- There is no incremental or persistent mode; every run is a full check.
- The diagnostic surface moved by 33 (1,186 → 1,153), so this is not a
  like-for-like comparison of identical work.

Performance history, methodology, and the reproduction recipe live in
[BENCHMARKS.md](BENCHMARKS.md); the detailed engineering investigations live in
[docs/perf/](docs/perf/) and are point-in-time records, not current state.

---

## What is deliberately not claimed

- **No full TypeScript compatibility claim.** The oracle gate establishes
  parity on the covered fixtures and projects only.
- **No general message-text or span/column parity claim.** Both strict sweeps
  are green across the 136 registered presets at this commit, which is a
  statement about those fixtures — not about arbitrary code. The strict flags
  stay non-gating so a new preset can record a drift without failing CI.
- **No claim that trpc matches `tsc`.** It is a workload and a measured
  baseline; 155 diagnostics still differ in each direction combined.
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

**Do not delete the build worktree until every measurement is finished.** The
generated default-lib subset resolves through `env!("CARGO_MANIFEST_DIR")`
(`crates/surge-ts-checker/src/default_lib/loader.rs`), so a binary built in a
throwaway worktree loses `generated-libs/` as soon as that worktree goes away —
copying the binary out first does not help. The pinned oracle
(`typescript@7.0.2`) ships no `lib*.d.ts` of its own, so fixtures that do not
pin `lib` depend on that fallback and collapse into `TS2304 Cannot find name
'Promise'` with only a `unknown lib 'es2024.full'` warning on stderr to say
why.

**Rule of thumb when updating this file:** do not change a number here unless
you ran the command that produces it. If you are recording someone else's
result, say so explicitly and cite the source document, commit, and date. If
you could not measure a number under valid conditions, say that — an absent
number is honest, a number measured on a loaded machine is not.
