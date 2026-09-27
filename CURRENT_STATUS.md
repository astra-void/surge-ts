# Current Status

**This document is the canonical description of what `surge-ts` is and does
right now.** Every other document in the repository is either a stable contract
([PUBLIC_API.md](PUBLIC_API.md)), a detailed measurement record
([REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md),
[BENCHMARKS.md](BENCHMARKS.md), [STRICT_DRIFT_INVENTORY.md](STRICT_DRIFT_INVENTORY.md)),
a design reference ([ARCHITECTURE.md](ARCHITECTURE.md)), or a point-in-time
engineering report ([docs/perf/](docs/perf/)).
Where any of them disagrees with this file about *current* behavior, this file
wins and the other document should be corrected.

Volatile numbers (preset count, test count, real-project parity, benchmark
medians) are recorded **here only**. Other documents link to this file rather
than copying them.

---

## Verification snapshot

| Field | Value |
| --- | --- |
| Date | 2026-09-27 |
| Commit | `6c1dfcf5` (clean checkout; release CLI built in a detached worktree at that commit, frozen copy passed to the harness with `SURGE_TS_BIN`) |
| Hardware | Apple M1 Pro (MacBookPro18,1), 10 cores, 16 GiB RAM |
| OS | macOS 27.0 (build 26A428) |
| Toolchain | rustc 1.98.1, Node v22.23.2, TypeScript oracle 7.0.2; harness scripts invoked through `pnpm run` |
| Build profile | cargo `release` (`lto = "fat"`, `codegen-units = 1`), default `mimalloc` allocator |

Commands run for this snapshot are listed in [§ How to re-verify](#how-to-re-verify).

**Is this snapshot current?** Compare the commit above with `git rev-parse
HEAD`. If they differ, the gate results below still describe the named commit
faithfully but may not describe `HEAD` — re-run the commands rather than
assuming they carry forward. Measure from a clean checkout: build the CLI in a
detached worktree at the commit you intend to cite and point the harness at it
with `SURGE_TS_BIN`, so an in-progress working tree cannot leak into a recorded
number. The sweep reads presets from the tree it runs in; for this snapshot the
primary working tree had no fixture edits.

### Gates

| Gate | Command | Result |
| --- | --- | ---: |
| Workspace tests | `cargo nextest run --workspace` | **not measured** for this snapshot. The last clean result recorded here was 1938 / 1939 at `f841633` (2026-09-07); it does not describe `6c1dfcf5`. |
| Oracle harness tests | `pnpm run oracle:test` | **23 / 23 passed** |
| Oracle preset sweep — normal gate | `pnpm run oracle:sweep -- --all --maxDiagnostics 200` | **913 / 919 passed** — 6 failures, see below |
| Oracle preset sweep — `--strictMessages` | same + `--strictMessages` | **848 / 919 passed** — the 6 above plus 65 message drifts |
| Oracle preset sweep — `--strictSpans` | same + `--strictSpans` | **898 / 919 passed** — the 6 above plus 15 span drifts |
| Oracle preset sweep — both strict flags | same + both | **836 / 919 passed** |
| Real-project gate — ky (exact 0/0) | `pnpm run real:ky:test` | **3 / 3 passed** |
| Real-project gate — unnamed (ceiling of 34) | `pnpm run real:unnamed:test` | **2 / 2 passed** — at **2** of 34 (see [§ Real-project compatibility](#real-project-compatibility)) |

The normal gate compares **diagnostic code counts** and **file/code/line**
parity against the upstream TypeScript compiler. Across all 919 presets the
sweep saw 4,999 `tsc` diagnostics and 5,012 surge diagnostics; every
file/code/line mismatch (6 `tsc`-only, 19 surge-only) is in the six failing
presets.

**The normal gate is not green.** Six presets fail it:
`base-constructor-signatures-basic`, `union-target-missing-property-basic`,
`javascript-script-globals`, `javascript-object-literals-open-ended`,
`javascript-object-literals-no-implicit-any` and
`javascript-this-assigned-members`. All six passed at the 2026-09-22 snapshot
(`7ea0cddb`) and already failed at `969d9abd` (2026-09-26); the change that
broke them has not been bisected.

**Strict drift grew with the preset count.** 77 presets carry message-text or
column drift at an already-correct file/code/line (65 message, 15 span, some
both), against 17 at the 2026-09-22 snapshot; most of the new ones are among
the 532 presets added since. The per-preset list is in
[STRICT_DRIFT_INVENTORY.md § Current snapshot](STRICT_DRIFT_INVENTORY.md#current-snapshot-2026-09-27).
The strict flags are *separate dimensions* — a preset can reopen one without
failing the normal gate — so their exit codes stay non-gating in CI.

`diagnostics-pack`, the compact emitted-diagnostic fixture, is green at **31/31**
and passes the normal gate *and* both strict gates.

### Upstream conformance

The checker benchmark also runs TypeScript's own conformance cases — the
held-out upstream tier of `pnpm run bench:checkers`, the cases that are in
neither surge's nor bolt-ts's test suite — and scores each diagnostic by
file/code/line against the oracle.

| Field | Value |
| --- | --- |
| Cases scored | 7,366 |
| True positives / false positives / false negatives | 25,031 / 502 / 1,965 |
| Precision / recall / F1 | 98.0% / 92.7% / **95.30%** |

**Carried forward, not re-measured for this snapshot:** these are the figures
recorded in the commit message of `6c1dfcf5` (2026-09-27). They were measured on
a working tree whose code was identical to `6c1dfcf5` (only documentation was
uncommitted), not from a clean checkout.

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
- A **bundled, version-pinned TypeScript standard library** embedded in the
  binary, so no `typescript` package, `node_modules`, `node`, or `npm` is
  needed at runtime. An on-disk lib directory is used only when explicitly
  selected (`--typescript-lib-path`, or `--physicalLibs` to discover the
  project's own install). `noLib: true` disables loading from any source. See
  [crates/surge-ts-checker/STANDARD_LIBS.md](crates/surge-ts-checker/STANDARD_LIBS.md).
- Declaration-side module resolution: relative imports, `paths`, `baseUrl`,
  package `exports` (conditional/pattern/subpath), package `imports`,
  `typesVersions`, package self-name, `types` / `typeRoots`, `@types`
  discovery under TS 6/7 semantics, and `/// <reference types>` directives
  followed recursively.
- Deterministic output: repeated runs render byte-identical diagnostics, and
  `--jobs 1` and `--jobs auto` produce identical diagnostics (worker results
  merge in loaded-file order, never completion order).
- An experimental, read-only compiler API modeled on TypeScript's
  (`surge_ts::api`, and the `surge-ts` npm package): see
  [docs/COMPILER_API.md](docs/COMPILER_API.md). Its agreement with
  typescript@6 (`scripts/api/compare-api.ts`) is **not measured** from a
  clean checkout yet; no figure is recorded.

### Non-goals

- Emit, transforms, declaration emit, or a language service.
- Incremental checking, watch mode, or project references.
- Full runtime/JS package resolution (`main` entrypoints, wildcard `exports`
  runtime conditions, `rootDirs`).
- Full `lib.d.ts` / DOM / Node / React parity.
- Being a drop-in replacement for `tsc` on arbitrary projects.

---

## Bundled standard library

surge embeds its own copy of TypeScript's `lib.*.d.ts` declarations, so a
release binary checks standard built-ins with no `typescript` package, `node`,
`npm`, or `node_modules` present. Architecture and lib-selection rules are in
[crates/surge-ts-checker/STANDARD_LIBS.md](crates/surge-ts-checker/STANDARD_LIBS.md).

| Field | Value |
| --- | --- |
| Bundled TypeScript | 7.0.2 (`crates/surge-ts-checker/generated-libs/manifest.json`) |
| Files vendored | 107 `lib.*.d.ts`, plus upstream `LICENSE.txt` and `NOTICE.txt` |
| Refresh command | `pnpm run lib:generate` |

Measured 2026-09-12 against the parent commit of this change, both built
`release` in detached worktrees and compared interleaved. The host was heavily
loaded by concurrent work, so CPU time and peak RSS are reported instead of wall
clock; wall-clock numbers taken that day are not trustworthy and are not
recorded.

| Measurement | Before | After |
| --- | --- | --- |
| Binary size | 6,540,640 B | 10,356,384 B |
| Tiny project, CPU | 0.030 s | 0.030 s |
| Tiny project, peak RSS | 34.4 MB | 43.8 MB |
| zod, CPU | 3.41 s | 3.39 s |
| zod, peak RSS | 381.5 MB | 390.1 MB |

The binary carries the 3.9 MB snapshot in read-only data. Peak RSS rises because
each loaded lib is copied out of that data into an owned `String`; keeping the
text borrowed would remove the copy and is unexplored.

Diagnostics did not move. All six real-project corpora (`trpc`, `zod`, `ky`,
`ofetch`, `tanstack-query`, `ts-pattern`) emit byte-identical output before and
after, even though each previously resolved its own installed TypeScript
(5.5.4, 5.9.2, 5.9.3, or 6.0.3) and now resolves the bundled 7.0.2 snapshot.

## Verified exact parity

"Exact parity" means: held at diagnostic code-count and file/code/line parity
with `tsc` by a gate that fails CI on regression. The authoritative list of
verified-exact feature areas and the stable embedding API is
**[PUBLIC_API.md](PUBLIC_API.md)** — treat anything not listed there as
unstable or out of scope.

Summary of what backs it today:

- 919 oracle presets under `tests/compat-projects/`, 913 of them green at the
  normal gate and 836 at both strict gates (see the table above).
- 1,173 compat-project fixtures in total; the ones not registered as oracle
  presets are exercised by `cargo nextest run --workspace` instead.
- `diagnostics-pack` at exact 31/31, pinning duplicate-declaration
  (TS2451/TS2393), TDZ (TS2448 + TS2454), missing-return span placement
  (TS2355/TS2366) and use-site generic-arity spans (TS2314/TS2315).

---

## Real-project compatibility

Measured with `pnpm run oracle:compare -- --project <tsconfig> --maxDiagnostics
100000` at the snapshot commit (`6c1dfcf5`, 2026-09-27) against the pinned
TypeScript 7.0.2 oracle, one corpus at a time. "tsc-only" and "surge-only" count
diagnostics, not locations, by file/code/line. Detailed history, drift
taxonomies, and burn-down records live in
[REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md).

| Project | Checkout | `tsc` | `surge-ts` | tsc-only / surge-only | Verdict |
| --- | --- | ---: | ---: | ---: | --- |
| **ky** (sindresorhus/ky 2.0.2) | `3419113` | 0 | 0 | 0 / 0 | **exact** — strict false-positive gate |
| **ofetch** (unjs/ofetch) | `1dbc37f` | 1 | 1 | 0 / 0 | **exact** — same file/code/line and message text (TS5108) |
| **zod** | `912f0f5` | 29 | 45 | 8 / 24 | **regressed** — exact 21/21 at the 2026-09-22 snapshot. The 8 `tsc`-only are all in `packages/zod/src/v3/tests/zzprobe.ts`, an untracked probe file an earlier session left in the checkout on 2026-09-24; it is not upstream source. The 24 surge-only are real: `TS2339` ×21 in `packages/zod/src/v3/tests/error.test.ts`, `TS2322` ×3 in `v4/classic/schemas.ts`, `v4/mini/schemas.ts` and a `node_modules` copy of the first. Not yet attributed to a commit. |
| **unnamed** (local Next.js App Router app) | `a5ece30` + 3 local edits dated 2026-09-01 | 0 | 2 | 0 / 2 | **inside the ceiling** (34) — 11 at the 2026-09-22 snapshot. `TS2353` at `app/[locale]/application/data-table.tsx:542` and `TS2344` in `components/ui/pagination.tsx`. |
| **trpc** | `dfbafa8` | 1244 | 1242 | 35 / 33 | a measured workload, **not** a parity claim — 34 / 8 at the 2026-09-22 snapshot. tsc-only by code: `TS7006` ×11, `TS2883` ×10, `TS2339`/`TS4111`/`TS2769`/`TS2554` ×2, one each of `TS4023`, `TS2345`. Surge-only: `TS18047` ×9, `TS18046` ×6, `TS2339` ×6, `TS2345` ×3, `TS2349` ×3, `TS2322`/`TS7006`/`TS18049` ×2. |
| **tanstack-query** (TanStack/query) | `cdbe8cb` | 0 | 15 | 0 / 15 | **provisional**, not a gate — 8 at the 2026-09-22 snapshot |
| **ts-pattern** (gvergnaud/ts-pattern 5.9.0) | `c92ca43` | 2 | 1 | 2 / 1 | **provisional**, not a gate — unchanged since 2026-09-13; the 2 `tsc`-only are 7.0.2-specific behaviour against assertions written for TypeScript 5.9 |
| **drizzle-orm** (drizzle-team/drizzle-orm 0.45.3) | `b786252` | 16 | 81 | 16 / 81 | **provisional**, not a gate — the first measurement since the 2026-09-17 deadlock was fixed (`8f46a6d8`). Surge-only by code: `TS7006` ×37, `TS2344` ×21, `TS2322` ×7, `TS2554` ×6, `TS2345` ×4, `TS2532` ×2, `TS2564` ×2, `TS2339` ×1. The 16 `tsc`-only are drizzle's own `@ts-expect-error` / `TS2769` pairs written against TypeScript 5.6. |
| **zustand** (pmndrs/zustand 5.0.15) | `2115efb` | 0 | 4 | 0 / 4 | **provisional**, not a gate — unchanged; `TS2349` ×2 and `TS2554` ×2, all in `tests/` |

Notes that matter:

- Projects where `tsc` reports zero diagnostics (ky, unnamed, tanstack-query,
  zustand) are false-positive corpora. Two of them are gates, of different
  strength: `pnpm run real:ky:test` asserts **exact 0/0**, and
  `pnpm run real:unnamed:test` asserts a **count ceiling of 34** (a ratchet that
  only moves down) plus a precondition that `tsc` still reports 0. Both *skip*
  cleanly when the project or the `typescript` package is absent (no third-party
  source is vendored).
- drizzle-orm and ts-pattern are aggregate targets (`tsconfig.surge.json`,
  installed by `pnpm run real:<name>:provision`) and are deliberately *not*
  clean-oracle corpora: TypeScript 7.0.2 reports diagnostics of its own there,
  against assertions the libraries wrote for older TypeScript.
- **zod and trpc regressed since the 2026-09-22 snapshot, and tanstack-query
  rose from 8 to 15.** The zod probe file should be removed from the checkout
  before the corpus is used as a gate again.
- trpc is a measured baseline and never a parity claim: `tsc` itself reports
  over a thousand diagnostics there, many from examples with unresolved
  workspace imports.
- The per-corpus burn-down narratives this section carried until 2026-09-22 were
  removed on 2026-09-27; `git show 6c1dfcf5:CURRENT_STATUS.md` has them.

---

## Known limitations

Each entry below carries the date it was last reproduced or refuted against the
oracle. Entries marked **2026-09-27** were re-probed at `6c1dfcf5` with a
reduced probe compared against TypeScript 7.0.2; anything older says so.

### Confirmed current gaps

- **A dynamic `import()` value is not typed.** *(reproduced 2026-09-27.)*
  `const s: string = (await import("./dep")).v` with `v` a `number` is not
  reported; `tsc` reports TS2322. `import("./dep").T` in a type position *is*
  checked.
- **A same-generic relation accepts a type argument surge reads as
  `unknown`.** *(reproduced 2026-09-27.)* `Box<string, unknown>` to
  `Box<number, any>` is not reported; `tsc` reports TS2322. surge treats an
  `unknown` source argument as a possibly uninferred one; only parameters
  annotated `in`/`out` are exempt.
- **An empty binding pattern checks nothing.** *(reproduced 2026-09-27.)*
  `const {} = undefined` is not reported; `tsc` reports TS2532. The pattern
  lowers to no declaration at all.
- **A generic interface's indirect back-edge still resolves to the degradation
  sentinel.** *(measured 2026-09-27.)* A direct self-reference
  (`interface L<T> { next: L<T> }`) is resolved lazily; a reference that reaches
  the interface through another declaration is not. Resolving those lazily too
  made zod 4.7× slower and drizzle-orm not finish, so it was not landed.
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
  though the complete, unmodified lib graph is bundled and loaded.

### Refuted on 2026-09-27

Each of these was listed as a current gap at the 2026-09-22 snapshot and no
longer reproduces on its reduced probe at `6c1dfcf5`.

- **`Promise<T>` modelled as `T`.** `const x: number = f()` with `f(): Promise<number>`
  reports TS2322 like `tsc`; the lib `Promise<T>` is nominal by default since
  `969d9abd`.
- **A contextually typed block-body callback keeping the sentinel.**
  `xs.map((x) => { return String(x); })` assigned to `number[]` reports TS2322.
- **An arrow argument's body inferred by the expression sketch.**
  `const bad: number = fn((n: number) => String(n))` reports TS2322 (the message
  renders the parameter as `(number) => string` rather than
  `(n: number) => string`).
- **`import("m").T` type queries.** `const bad: import("./dep").T = { a: 1 }`
  reports TS2322 at the member.
- **Generic JSX components.** `<List<string> items={[1]} />` reports TS2322.

### Refuted on 2026-09-22

Each of these was listed as a current gap and no longer reproduces on its
reduced probe at `7ea0cddb`. A reduced probe is not the corpus: where the entry
named a corpus trigger, that trigger was not re-measured separately.

- **Overload resolution.** A call matching no overload of a declared function
  group now reports `TS2769` like `tsc`, and an interface or type-literal method
  group is resolved member by member (`2a22babc`) rather than through the
  group's permissive fold.
- **Module augmentation lost through a star re-export wrapper.** With `declare
  module "core"` in a `.d.ts` and `export * from "core"` in `wrapper`, the
  augmented member is checked when the interface is imported from `wrapper`.
- **An augmented base interface not seen through a derived one.** With
  `declare module "./generated" { interface BaseNode { parent: Node } }` and
  `interface Identifier extends BaseNode`, `id.parent` resolves and is checked.
- **`typeof import("pkg")` in a type alias.** `type T = typeof
  import("./dep")` binds, and `T["x"]` is checked.

Older refutations (2026-09-01 and 2026-09-03) were removed on 2026-09-27;
`git show 6c1dfcf5:CURRENT_STATUS.md` has them.

---

## Experimental and feature-gated behavior

Runtime behavior gates use the `SURGE_` environment-variable prefix. Nothing
here is part of the stable surface; gates may be removed once their default is
settled.

**Opt-in (off by default) — changes checking behavior or performance:**

Gates are classified by what they are: a **semantic compatibility gate** hides
behavior surge should ultimately perform by default and is therefore a bug to
be burned down, not a feature; a **performance experiment** is an
implementation strategy that need never be enabled; a **kill switch** restores
a previous implementation of behavior that is already the default; and
**instrumentation** is opt-in by nature.

**Semantic compatibility gates (off by default).** Each of these is intended
TypeScript behavior that surge does not yet perform in the default
configuration. Every one carries the concrete reason it is still off.

| Gate | Effect | Why still off |
| --- | --- | --- |
| `SURGE_UPGRADE_ANALYSIS_SCOPES=1` | Give a module's exported declarations the real resolution scope (own declarations + import layers) during analysis instead of the import-less preliminary one. Makes the published type of an export honest where its body names an imported type. | On trpc it closes 12 `TS2339` and opens 13: an honest export lets `ProtectedIntersection`'s conditional decide from a router key set surge never had, and the string-literal error type it then produces swallows every property read off the router. See [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-tsc-only-inventory-2026-09-11). |
| `SURGE_GENERIC_RECURSIVE_ALIAS=1` | Key a generic alias's resolution frame by its *instantiation* (declaration + resolved-argument fingerprint) rather than its declaration, so recursing into itself with different arguments is not read as a cycle. Without it a recursive generic record collapses to the degradation sentinel. | One false positive and a cost, both on named corpora. Measured 2026-09-14 on all nine corpora from an isolated worktree: zustand 37 → 11 (27 false positives closed, **1 opened** — `TS2554` at `tests/middlewareTypes.test.tsx:476`, where `devtools`' three-argument `set` loses its mutator extension under the nested `devtools(subscribeWithSelector(…))` composition), every other corpus byte-identical. The previously recorded drizzle-orm 4 → 6 and tanstack-query +2 regressions **no longer reproduce**. The nontermination that used to seal this gate is fixed by the per-root breadth budget (see below); ts-pattern now finishes, but costs seconds where the gate-off run costs a third of one, because the gate makes surge actually evaluate a type-level program it previously short-circuited to `unknown`. |
| `SURGE_LOCAL_TYPE_DECLARATION_CHECKS=1` | Check a body-local `type` / `interface` / `class` declaration at its statement, the way a top-level one is checked. | Measured 2026-09-14 on all nine corpora: only ts-pattern moves, 0 → 22, every one an `Expect<Equal<…>>` type-level divergence. Eight other corpora are byte-identical, so the remaining work is entirely ts-pattern's type-level program. |
| `SURGE_COMPLETE_DEFAULT_ARGS=1` | Carry an interface's resolved defaults on its reference, as tsc's type reference does; positional `infer` binding against a partially-written reference needs them. | Measured 2026-09-14: a pure regression of 4 on the current corpora (zod +2, tanstack-query +1, trpc +1) and 0 closed. zod's is `ParsePayload`'s bare `$ZodRawIssue` member losing its own alias default once the interface reference carries one — a default bound under the wrong substitution. |
| `SURGE_VALUE_EXPORT_REFINEMENT=1` | Re-analyze a module whose value *or type-only* imports settled in the previous round, so an export whose initializer reads an imported value is published with its real type instead of the sentinel (up to 8 rounds). | Correct but no gain yet, and costly (recorded in `6dda508d`, 2026-09-21, and the gate's doc comment): trpc is byte-identical with it on, and zod goes from about 7 s to 53 s wall. The trpc `TS7006` it was once credited with closing were an artifact of a missed relative-import dependency; what blocks them is the router's root types (`ctx`, `errorShape`, `transformer`). |
| `SURGE_TSC_IMPLICIT_ANY=1` | Apply tsc's implicit-any rule without surge's two extra exemptions (a degraded contextual type, unmodelled JSX props). | The exemptions stand in for contextual types surge fails to supply. Recorded 2026-09-16 in the gate's doc comment: trpc tsc-only 50 → 37, but surge-only trpc 7 → 724, zod 0 → 357, tanstack-query 7 → 211 — dominated by zod's `$constructor` interface losing its construct signature. |
| `SURGE_DISTRIBUTE_REFERENCE_UNIONS=1` | Distribute an intersection over a union operand that arrives as a *reference* (`ComponentType<P> & {…}`), not only over a bare union. | Recorded 2026-09-16 in the gate's doc comment: trpc tsc-only 55 → 70, surge-only 2 → 4, because an intersection merge keeps only one of an operand's overloaded call signatures (jscodeshift's `JSCodeshift` becomes uncallable). |

### Termination model for recursive generic aliases

Under `SURGE_GENERIC_RECURSIVE_ALIAS=1` a generic alias's resolution frame is
keyed by declaration *plus* a fingerprint of its resolved arguments, so a
recursion whose arguments keep changing resolves instead of being cut as a
cycle. Termination then rests on three things, in order of precedence:

1. **Repeated-state detection** — the frame key collides exactly when the
   argument tuple repeats, which is the real cycle. This is the semantic model.
2. **Path-local depth guards** — `MAX_RESOLUTION_DEPTH` (24) and
   `MAX_NESTED_INSTANTIATIONS` (8) bound how deep one path may go.
3. **A per-root breadth budget** (`MAX_ROOT_INSTANTIATION_WORK`, 200) — the
   last-resort ceiling, overridable with `SURGE_ROOT_INSTANTIATION_WORK`.

(3) exists because (1) and (2) are both path-local. A type-level program that
*fans out* — ts-pattern's `Chainable` / `Pattern` family instantiate a fresh
argument tuple per member per level — never repeats an argument tuple and never
nests deeply, so neither fires while total work grows exponentially. Before the
budget, ts-pattern was SIGKILLed at 600 s with the gate on.

Two things are worth recording so they are not re-derived. Profiling put ~88% of
that runtime in `Type` structural equality under instantiation-cache lookup,
which reads like a bucket-scan problem; it is not — forcing the per-declaration
bucket cap to 1, 4 and 16 all still time out at 240 s. And the ceiling's value
is empirical but not arbitrary: the cost is superlinear in it (200 → ~5 s,
500 → 14 s, 1000 → 35 s, 2000 and 5000 → still running at 240 s), while
dropping to 50 terminates sooner but costs zod its exact 21/21 parity.

`SURGE_ROOT_WORK_TRIP=1` names the declarations that exhaust the budget.

**Known gap:** the nontermination has no reduced reproduction. Three synthetic
fan-out probes (a variadic-tuple walk, a per-key recursive `Exclude`, and a
direct transcription of `Chainable`) all terminate in under 0.1 s and none even
reach the budget, so the only reproduction is ts-pattern itself, via
`SURGE_GENERIC_RECURSIVE_ALIAS=1 SURGE_ROOT_INSTANTIATION_WORK=5000` on the
provisioned corpus. A regression test asserting the budget is therefore *not*
included: one written against those probes would pass with and without the fix.

**Performance experiments (off by default).** These are implementation
strategies, not semantics; leaving them off costs no compatibility.

| Gate | Effect |
| --- | --- |
| `SURGE_LAZY_IFACE_MEMBERS=1` | Stage 1 of member-level lazy interface expansion (property annotations only; methods stay eager). Design: [docs/perf/MEMBER-LAZY-EXPANSION.md](docs/perf/MEMBER-LAZY-EXPANSION.md). |
| `SURGE_LIB_MEMBER_CACHE=1` | Extended per-member instantiation cache. Measured at roughly +2% and left off. |
| `SURGE_IFACE_CACHE_ALL=1` | Extended interface-instantiation cache. Measured at +1.7% and left off. |
| `SURGE_DEFER_PLACEHOLDER_INSTANTIATIONS=1` | Defer placeholder-argument instantiation. Left off: mixed intersections and conditionals peel immediately and zod's RSS rose. |

**Default-on with an opt-out kill switch** (for A/B profiling and regression
isolation only — production behavior is the default). Four of them changed
checking recently enough to need their own note:

| Gate | Default since | Default behavior | Opt-out |
| --- | --- | --- | --- |
| `SURGE_PROMISE_NOMINAL` | `969d9abd`, 2026-09-26 | The lib's `Promise<T>` / `PromiseLike<T>` stay the interfaces they are instead of collapsing to the awaited `T`: an async body is promised, promise instantiations relate by `T`, and a promise used as its value is reported. | `=0` restores the collapse |
| `SURGE_INFER_BLOCK_RETURN_TYPES` | `7ea0cddb`, 2026-09-22 | An unannotated block-bodied arrow or function expression takes its return type from its `return` statements (tsc's `getReturnTypeFromBody`), widened unless a contextual type asks for the literal. Before, every such function was the sentinel and no use of it was checked. Not applied when a contextual type exists (see [§ Known limitations](#confirmed-current-gaps)). | `=0` |
| `SURGE_INFER_DECLARATION_RETURN_TYPES` | `d8e156eb`, 2026-09-21 | `lazy`: an unannotated declaration's return type, and an unannotated class field's or getter's type, are read from the body on first demand, like tsc's `getReturnTypeOfSignature`. Generic classes keep `any` for now. | `=0` or `=off` restores sentinel returns and `any` members; `=lazy:<substring>` limits it to matching files; `=1` (or any other value, read as a file substring) runs the eager walk during signature collection, kept for comparison only |
| `SURGE_EARLY_MODULE_LOCAL_VALUES` | `2c25f3f4`, 2026-09-17 | Module-local value tables are seeded before export publication, so a `typeof <value>` inside a value export's type resolves instead of dying at the sentinel. | `=0` |

The commit messages record what each flip cost: `d8e156eb` about +3–4% RSS
and +5% wall on trpc, `7ea0cddb` trpc timing within noise. Those figures are
carried forward from the commit messages (2026-09-21 and 2026-09-22); they were
not re-measured for this snapshot.

The rest: `SURGE_AMBIENT_BLOCK_IMPORTS`,
`SURGE_NS_IFACE_MERGE`, `SURGE_NS_QUALIFIED_RETRY`, `SURGE_READONLY_ARRAYS`,
`SURGE_VARIADIC_TUPLES`, `SURGE_WRITTEN_CALL_SIGNATURE`, `SURGE_LAZY_DTS_VALUES`,
`SURGE_THIN_PRELIM`, `SURGE_MODULE_TYPE_DEDUP`, `SURGE_EAGER_DEPENDENCY_ALIASES`,
`SURGE_EAGER_REFERENCE_INTERSECTIONS`, `SURGE_IFACE_KEY_ENV`,
`SURGE_PRESCANNED_PARSE_REUSE`, `SURGE_PRELIM_EARLY_RELEASE`,
`SURGE_PARALLEL_ANALYSIS_FINAL`, `SURGE_DISABLE_CANONICAL_*`,
`SURGE_DISABLE_SIG_CONTEXT_CACHE`, `SURGE_DISABLE_PHYSICAL_INTERFACE_*`.

`SURGE_AMBIENT_BLOCK_IMPORTS` became the default on 2026-09-14. It binds an
import written inside a `declare module "..."` block, which is what TypeScript
does. It had been opt-in because resolving the @types/node graph those imports
open up cost +36% user time on trpc; re-measured on an interleaved A/B with one
frozen binary, trpc user time is 4.07 s off versus 3.98 s on with peak RSS
unchanged, zod is within noise, and the diagnostic set is identical on all nine
corpora.

**Instrumentation only** (no semantic effect): `SURGE_TIMINGS`, `SURGE_RSS`,
`SURGE_RETENTION_CENSUS`, `SURGE_PAUSE_AT_STAGE`, `SURGE_ALLOCATION_CENSUS`,
`SURGE_TYPE_GRAPH_CENSUS`, and the various `SURGE_TRACE_*` / `SURGE_*_STATS`
gates. The hidden CLI flags `--timings` and `--rss` set the first two.

**Memory guard** (no semantic effect unless it fires): `SURGE_MAX_FOOTPRINT_MB=<n>`
makes the `surge` binary exit with status 137 once its physical footprint
(resident set where the platform has no footprint counter) exceeds `n` MiB,
naming the last stage boundary on stderr. macOS enforces neither `RLIMIT_AS`
nor `RLIMIT_RSS` and has no per-process OOM killer, so a runaway type
expansion otherwise takes the whole machine down; set it in harness runs.
Off by default.

---

## Current performance state

One workload, one machine, one pair of commits. **These numbers do not
transfer** across projects, hardware, allocators, or build profiles, and they
are not a compiler comparison.

Release binaries built from clean detached worktrees at the previous snapshot
(`7ea0cddb`, 2026-09-22) and this one (`6c1dfcf5`), run as three interleaved
pairs per project on 2026-09-27, project mode, default `--jobs`, cold process
over a warm filesystem cache, peak resident set size from `/usr/bin/time -l`.
The load average was 6–10 during the run, so absolute wall figures compare only
within this table.

| Project | Wall, median | Peak RSS, median |
| --- | ---: | ---: |
| tRPC monorepo (`.local-projects/trpc`, `dfbafa8`) | 24.8 s → **77.2 s** | 1.11 GB → **2.29 GB** |
| zod (`912f0f5`) | 7.3 s → **12.8 s** | 722 MB → **1,119 MB** |

**This is a large regression:** 3.1× wall time and 2.1× memory on tRPC, 1.75×
and 1.55× on zod, since the previous snapshot. It has not been bisected.
`c04652cc` (2026-09-26) already takes about 77 s on tRPC (a corpus run recorded
in the message of `040713d3`), so the growth happened between 2026-09-22 and
2026-09-26; the per-commit A/Bs recorded since each measured a small step.

The older interleaved record (2026-09-07, 5.4–5.7 s on tRPC) was taken in a
different round and is not comparable to this table; `git show
6c1dfcf5:CURRENT_STATUS.md` has it. Methodology and the reproduction recipe live
in [BENCHMARKS.md](BENCHMARKS.md); the detailed engineering investigations live
in [docs/perf/](docs/perf/) and are point-in-time records, not current state.

---

## What is deliberately not claimed

- **No full TypeScript compatibility claim.** The oracle gate establishes
  parity on the covered fixtures and projects only, and the upstream
  conformance F1 is a statement about that suite, not about arbitrary code.
- **No general message-text or span/column parity claim.** The strict sweeps
  carry drift on 77 of the registered presets at the snapshot commit (see
  [§ Gates](#gates)). The strict flags stay non-gating so a preset can record a
  drift without failing CI.
- **No claim that trpc matches `tsc`.** It is a workload and a measured
  baseline; 68 diagnostics differ at the snapshot commit, 35 `tsc`-only and 33
  surge-only.
- **No wall-clock performance claim at this commit** — see
  [§ Current performance state](#current-performance-state).
- **No cross-tool performance claim.** `pnpm bench:compilers` exists as a
  developer aid; its output is local-machine-relative and is not a marketing
  comparison.
- **No suppression-transparency claim yet.** ky's source-level parity is 0/0,
  but three suppression counters (`suppressedRustOnly`,
  `suppressedDeclaration`, `externalModuleStubs`) sit behind that number. They
  were audited on 2026-06-20; the audit document was removed on 2026-09-27 and
  git history keeps it.

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
| What did the project look like earlier? | git history — version-tagged notes were removed on 2026-09-27 |

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

```bash
pnpm run oracle:compare -- --project .local-projects/trpc --maxDiagnostics 100000 --json
```

```bash
pnpm run bench:checkers -- --upstream --upstreamLimit 0 --jobs 8 --maxMemory 1024
```

The `oracle:compare` command is repeated per corpus (`ofetch`, `zod`, `zustand`,
`tanstack-query/tsconfig.surge.json`, `ts-pattern/tsconfig.surge.json`,
`drizzle-orm/drizzle-orm/tsconfig.surge.json`, and `../../nextjs/unnamed`), one
at a time. The checker benchmark compares against bolt-ts as well; pass a stub
binary with `--boltBin` when only surge is being measured. For this snapshot every command ran with `SURGE_TS_BIN` pointing at
a frozen copy of the release binary built in a detached worktree at the
snapshot commit, and `SURGE_MAX_FOOTPRINT_MB` set as a guard. Without
`SURGE_TS_BIN` the scripts build the CLI from the tree they are run in, which is
the working tree, not the commit you mean to cite. The sweep also reads presets
from that tree; if the working tree has fixture edits, compare against
fixtures extracted from the commit (`git archive <commit> tests/compat-projects`).

The strict sweeps exit non-zero by design when drift exists; that is the
expected outcome, not a failure of the run. `--strictMessages --strictSpans`
must be passed as two separate arguments; the sweep rejects them quoted as one.

Real-project targets are gated on the checkout being present locally
(`.local-projects/` is gitignored and no third-party source is vendored). When
a project is absent, record it as *not measured* rather than carrying forward
an older number as current.

**Binaries no longer depend on their build tree.** The standard library is
embedded at build time (`crates/surge-ts-checker/build.rs`), so a binary copied
out of a throwaway worktree keeps working after that worktree is deleted. This
replaced an `env!("CARGO_MANIFEST_DIR")` lookup that made a moved or deleted
source tree collapse into `TS2304 Cannot find name 'Promise'`, with only an
`unknown lib 'es2024.full'` warning on stderr to say why.

**Rule of thumb when updating this file:** do not change a number here unless
you ran the command that produces it. If you are recording someone else's
result, say so explicitly and cite the source document, commit, and date. If
you could not measure a number under valid conditions, say that — an absent
number is honest, a number measured on a loaded machine is not.
