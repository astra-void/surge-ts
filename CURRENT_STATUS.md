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
| Commit | `f841633` (clean checkout; built and measured from a detached worktree at that commit) |
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

**The regression this snapshot opened is closed.** tRPC's peak memory footprint
and wall time roughly doubled at `4b3bcc0` and are back to their pre-`4b3bcc0`
level at this commit; the bisect and the before/after are in
[§ Current performance state](#current-performance-state).

### Gates

| Gate | Command | Result |
| --- | --- | ---: |
| Workspace tests | `cargo nextest run --workspace` | **1932 / 1933 passed** — `literal_union_does_not_collapse_boolean_literal_with_boolean` asserts that `true | false` stays a two-member union, which `5b87a462` deliberately changed without updating the test |
| Oracle harness tests | `pnpm run oracle:test` | **23 / 23 passed** |
| Oracle preset sweep — normal gate | `pnpm run oracle:sweep -- --all --maxDiagnostics 200` | **225 / 225 passed** |
| Oracle preset sweep — `--strictMessages` | same + `--strictMessages` | **224 / 225 passed** — one message drift, see below |
| Oracle preset sweep — `--strictSpans` | same + `--strictSpans` | **225 / 225 passed** |
| Oracle preset sweep — both strict flags | same + both | **224 / 225 passed** — the same drift |
| Real-project gate — ky (exact 0/0) | `pnpm run real:ky:test` | **3 / 3 passed** |
| Real-project gate — unnamed (ceiling of 34) | `pnpm run real:unnamed:test` | **2 / 2 passed** — at 0 of 34 |

The normal gate compares **diagnostic code counts** and **file/code/line**
parity against the upstream TypeScript compiler. Across all 225 presets the
sweep saw 342 `tsc` diagnostics and 342 `surge-ts` diagnostics, with
`onlyTsc = 0` and `onlyRust = 0`.

**Span/column matches on every registered preset; message text matches on all
but one.** `declaration-flow-narrowing-basic` drifts under `--strictMessages`:
surge names an assigned union `'"a" | "b"'` where tsc names it `'string'`, same
file/code/line/column. It is the only drift in the sweep, it predates the
2026-09-13 measurement, and the normal gate is unaffected. The strict flags are
*separate dimensions* — a preset can reopen one without failing the normal gate
— so their exit codes stay non-gating in CI. The 2026-09-01 drift inventory and
the fixes that closed it are kept in
[STRICT_DRIFT_INVENTORY.md § Current snapshot](STRICT_DRIFT_INVENTORY.md#current-snapshot-2026-09-03).

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

- 225 oracle presets under `tests/compat-projects/`, all green at the normal
  gate, and all but one at the strict gates (see the table above).
- 447 compat-project fixtures in total; the ones not registered as oracle
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
| **zod** | `912f0f5` | 21 | 21 | **exact** — same file/code/line and message text; the two surge-only `TS18046` the `trpc-fn-80` merge opened were closed by `2067888`, which gives a generic class a permissive static object when it declares a predicate or assertion static, so `z.ZodError.assert` narrows again |
| **unnamed** (local Next.js App Router app) | local | 0 | 0 | **exact** — strict false-positive corpus |
| **trpc** | `dfbafa8` | 1244 | 1155 | surge-only **0**, `tsc`-only 89 — a false-positive gate with an inventoried false-negative side, **not** a parity claim (dirty-tree measurement, 2026-09-13, after the `trpc-fn-80` merge) |
| **tanstack-query** (TanStack/query) | `cdbe8cb` | 0 | 10 | **provisional** — false-positive burn-down list measured on a dirty tree, not a gate (see note) |
| **ts-pattern** (gvergnaud/ts-pattern 5.9.0) | `c92ca43` | 2 | 1 | **provisional** — 446 when first measured; the 1 that remains is surge-only, and the 2 `tsc`-only reports are 7.0.2-specific behaviour (union member order; `unknown` for a predicate inside `P.array`) that surge does not reproduce — see the 2026-09-13 note in REAL_PROJECT_COMPAT.md; under `SURGE_LOCAL_TYPE_DECLARATION_CHECKS=1` (body-local `Expect<Equal<…>>` assertions fire) the surge-only count is 7 after the 2026-09-13/14 follow-up (63 → 7; the six left are one `Chainable` alias-cycle degradation, see the note); dirty-tree measurement, not a gate (see note) |
| **drizzle-orm** (drizzle-team/drizzle-orm 0.45.3) | `b786252` | 16 | 4 | **newly provisioned, provisional** — 134 when first measured; all 4 are surge-only and the 16 `tsc` reports are unmatched; not a gate (see note) |
| **zustand** (pmndrs/zustand 5.0.15) | `2115efb` | 0 | 136 | **newly provisioned, provisional** — clean under the oracle, so every surge diagnostic is a false positive; first measurement, not a gate (see note) |

Notes that matter:

- Projects where `tsc` reports zero diagnostics (ky, unnamed) are
  false-positive regression corpora, but the two gates differ in strength.
  `pnpm run real:ky:test` asserts **exact 0/0** — any surge diagnostic fails it.
  `pnpm run real:unnamed:test` asserts a **count ceiling of 34** over-reports
  (a ratchet that only moves down) plus a precondition that `tsc` still reports
  0. Both *skip* cleanly when the project or the `typescript` package is absent
  (no third-party source is vendored).
- **drizzle-orm is newly provisioned and is not a gate.** The aggregate target
  `tsconfig.surge.json` (448 `src/` files plus the 80-file `type-tests/` suite,
  installed by `pnpm run real:drizzle-orm:provision`) is the corpus — the same
  set upstream's own `test:types` covers, under the repo's unusually strict
  settings (`noUncheckedIndexedAccess`, `noPropertyAccessFromIndexSignature`,
  `noImplicitOverride`, `checkJs`). It is deliberately *not* a clean-oracle
  corpus: the pinned TypeScript 7.0.2 oracle reports 16 diagnostics of its own,
  eight `@ts-expect-error` directives drizzle wrote against TypeScript 5.6 that
  TypeScript 7 no longer satisfies at that line, each paired with the `TS2769`
  that landed one construct over. surge matches none of them. surge reported
  **134** on the first measurement, every one surge-only, and finished in about
  2.4 s at about 480 MB peak RSS (`tsc` 12.8 s / 1.22 GB, `tsgo` 2.6 s /
  1.66 GB on the same target) — no hang and no unbounded expansion, so the
  corpus was usable as-is. **52 of those closed in one pass** and the corpus
  stands at **82**: drizzle's `is(value, Klass)` entity guard is a generic
  predicate narrowing to `InstanceType<T>` with `T` inferred from the *class
  value* passed alongside, and a generic class's value side is deliberately
  `any`, so `T` bound to `any` and the guard either replaced the subject with
  `any` or proved nothing. A constructor surface over the class's instance type
  is now stood up for type-argument inference only — the value's own type is
  untouched, so the measurement that sealed the `any` is not reopened. Closing
  it exposed one new over-report, a constructor parameter property written with
  a default read as optional, which was closed in the same pass. Both are pinned
  (`generic-class-entity-guard-basic`,
  `parameter-property-default-required-basic`) and every other corpus — ky, zod,
  ofetch, trpc, tanstack-query, ts-pattern — is byte-identical across the
  change. **A second pass took it to 49**, five more causes, each pinned by a
  preset and each leaving every other corpus byte-identical: a
  `/// <reference types="pkg" />` resolved to a package's `index.ts` instead of
  its `index.d.ts`, so `@cloudflare/workers-types` contributed no globals at all
  (−17); a bodyless class member — `abstract`, an overload signature, an ambient
  class's — was run through the function-body check and reported a missing
  return (−2); `T[string]` was rejected outright instead of reading the
  receiver's string index signature (−2); the inline-arrow predicate a `filter`
  call reads was resolved with reporting on, out of the arrow's own scope (−2);
  and a union member a property-path guard rules out was kept rather than
  dropped, which is the AWS SDK `?: never` member pattern (−10). It stands at **4**
  after five more passes. The third: the five `TS2344` `20b5ef3` had put on this corpus are
  suppressed (a constraint stated in terms of a sibling parameter is only as
  right as surge's model of that sibling), `new SQL(…)` constructs again where a
  generic class merges with a namespace (−7), and `drizzle(client)` is callable
  again where a function does (−11). None of those three is preset-pinned —
  each was reduced as far as the corpus file and no further, and every
  hand-written version of the shape is already clean — so the corpus is the
  only evidence for them. The fourth closed the cluster this burn-down had
  parked as blocked: a shape was refused as a type-argument candidate when the
  sentinel appeared anywhere inside it, *including in a member's own signature*,
  and `class SQL implements SQLWrapper` with `SQLWrapper.getSQL(): SQL` is
  mutually recursive, so surge's cycle break left every drizzle class that
  implements it unusable (−10). The same pass typed an unreachable `typeof`
  branch as `never` rather than leaving the union standing, which is the seven
  libsql entry points (`typeof-guard-unreachable-branch-basic`). A fifth closed the four
  `.dbMigrations` — `await` is erased at parse time, so a class that *implements*
  `Promise<T>` reached the index as the raw query object
  (`thenable-awaited-index-access-basic`) — and restored the guard for a
  predicate whose subject is a property path, which had been dropped outright
  (`predicate-property-path-subject-basic`).

  A sixth closed the five `TS7006` on an
  immediately-invoked arrow (`immediately-invoked-arrow-parameters-basic`) —
  which needed no AST work at all: the callee expression is already carried by
  `ParsedExpression::ExpressionCall`, and only the checking treated the arrow as
  a standalone expression with no expected type.

  **The 4 that remain each have an identified cause.** Two `.config` on
  `Relation<string>` are a predicate guard whose subject is an *element access*:
  element guards model truthiness, `typeof` and nullish, and have no predicate
  arm. Adding one was **implemented and rejected by measurement** — it closes the
  first of the two and turns the second into two `TS2532`, because drizzle's
  shape (`is(rs[0], One) && rs[0].config`) needs the next layer immediately: a
  property path *below* an element access. The element key renders the path
  before the index (`a.b[0]`), so `a[0].b` is not expressible, in the writer or
  in any reader. Closing these two is that key-format extension, not the
  predicate arm. One `'result.typings' is possibly 'undefined'` is the
  `if (!x.y) { x.y = … }` join — an assignment in the branch makes the property
  definite on both paths. One `Property 'strings' does not exist on type
  'TemplateStringsArray'` in `sql.ts` is unreduced.

  One known gap is *not* in that four: `await` is still erased at parse time, so
  a user-defined thenable reads as the value it resolves to only where the
  receiver is normalised for indexing — `rows.length` on the same value still
  reports, and no corpus exercises it. Representing `await` in the AST would fix
  it precisely, at the cost of teaching every structural matcher in narrowing and
  flow to peel the new node; erasure is what makes `await x` narrow exactly like
  `x` today. Measured from an isolated worktree, base and branch built from
  the same tree, but not from a clean checkout of a commit that contains these
  changes; re-measure before treating any of it as a baseline. Full inventory in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#drizzle-orm-corpus-provisioned-2026-09-13).
- **ts-pattern is newly provisioned and is not a gate.** The aggregate target
  `tsconfig.surge.json` (18 `src/` files plus the 48-file type-level test suite,
  installed by `pnpm run real:ts-pattern:provision`) is the corpus. It is
  deliberately *not* a clean-oracle corpus: the pinned TypeScript 7.0.2 oracle
  reports 2 diagnostics of its own, both `TS2344` where a
  `Expect<Equal<…>>` assertion the library wrote against TypeScript 5.9 now
  evaluates to `false`. Neither is matched by surge. surge reported **446** when
  the corpus was first measured, and **431 of those were one root cause**:
  overloaded members of a *type literal* were not grouped — the last declaration
  won and every earlier overload was dropped — so every
  `match(input).with(pattern, handler)` was checked against a three-parameter
  overload and reported `TS2554`. A type literal now folds a repeated function
  member the way an interface body already did, pinned by the
  `type-literal-method-overloads-basic` preset. A second pass closed twelve more
  — a symbol-keyed indexed access, five `infer` captures inside type predicates,
  `typeof` narrowing of an element access, and five array literals picking their
  member out of a union target — taking the corpus to **7**, and a third closed
  six of those: an object literal now picks its union member by the discriminant
  it writes, and `as const` is finally applied on the inference path, where it
  had been ignored entirely. The corpus stands at **1**. No new diagnostic
  appeared anywhere across any of the three passes: ky, ofetch, zod, unnamed and
  trpc are byte-identical throughout and the tanstack-query aggregate improved.
  Typing an object literal against each union member speculatively was
  implemented, measured, and **rejected** (5 false positives on trpc, and
  tanstack-query did not finish in ten minutes); the member-picking that landed
  reads the discriminant for free instead, under a measured cost bound of 20 own
  properties per candidate. A fourth pass fixed two of the three layers behind
  the last one — a type predicate declared on a *later* overload was invisible to
  guard narrowing, and a generic predicate's type arguments were inferred from
  the tested argument alone, so anything stated in terms of another argument fell
  back to `any` and narrowed to the wrong member. Both are pinned
  (`overload-group-type-predicate-basic`,
  `predicate-type-argument-from-arguments-basic`) and both leave every corpus
  byte-identical, because the third layer is the blocked overload program:
  ts-pattern's pattern argument comes back as `any` from its own overload group.
  The one that remains, and the two `tsc`-only assertions with the five-layer
  stack under them (variadic tuples, contravariant inference, last-overload
  inference, `Equal` identity, `TS2344` on type-reference arguments — all landed
  2026-09-13, corpus-neutral), are inventoried in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#ts-pattern-surge-only-inventory-2026-09-11).
  Measured on a **dirty working tree** on top of `6e034fd`; re-measure from a
  clean worktree before treating any of it as a baseline.
- **zustand is newly provisioned and is not a gate.** Upstream's own root
  `tsconfig.json` is the corpus — `pnpm test:types` runs `tsc --noEmit` against
  it, it covers `src/` and `tests/` in one 31-file program, and the pinned
  TypeScript 7.0.2 oracle reports **0** diagnostics on it, so it is a
  false-positive corpus like ky and unnamed and needs no aggregate target
  (`pnpm run real:zustand:provision` clones and installs it). surge reported
  **136** on the first measurement, every one surge-only, finishing in about
  0.24 s at about 88 MB peak RSS (`tsc` 1.71 s / 326 MB, `tsgo` 0.19 s / 181 MB
  on the same target) — no hang and no unbounded expansion. It is the smallest
  corpus in the set and the most concentrated: zustand's middleware typing is
  built on a `declare module 'zustand/vanilla'` augmentation of
  `StoreMutators`, and the three augmentation members resolve in the *augmented*
  file's scope rather than the augmenting file's, so `WithDevtools`,
  `WithPersist` and `WithRedux` are `TS2304` at their use sites in
  `src/vanilla.ts` and the store type they feed loses its middleware surface.
  The inventory and the two reduced reproductions are in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#zustand-corpus-provisioned-2026-09-13).
  A ky-style exact-0/0 gate can be armed once the over-report reaches zero; it
  is deliberately not armed at 136.
- **tanstack-query is newly provisioned; its count is provisional.** The
  aggregate target `tsconfig.surge.json` (10 TS/React packages, installed by
  `pnpm run real:tanstack-query:provision`) is clean under the oracle, so it is
  a false-positive corpus like ky and unnamed. Until 2026-09-10 `surge` could
  not finish it: `Window & typeof globalThis` names itself through `window` and
  `self`, and the eager intersection merge unfolded it without bound (a stack
  overflow unbounded, 55 GB of RSS under a depth bound). The merge now detects
  the cycle by operand identity, flattens nested deferred intersections, and
  shares one deferred merge per operand set, and `typeof globalThis` is a
  nominal reference. With that the aggregate completes in about 0.6 s at about
  190 MB peak RSS, after the program-lifetime interface memo
  ([docs/perf/TANSTACK-QUERY-PROGRAM-MEMO-2026-09-12.md](docs/perf/TANSTACK-QUERY-PROGRAM-MEMO-2026-09-12.md)).
  The over-report has been burned down since — **10** at the 2026-09-13
  measurement, from 133 when it first completed — and every one of them is a
  false positive. The last three closed after the `trpc-fn-80` merge: a package
  re-export's `Array<string>` return indexed as a missing property
  (`package-reexport-array-index-basic`), and `!!query` as an `&&` operand —
  aliased or inline — proving nothing (`alias-double-negation-and-basic`). After the `react-query` pass below, four more closed in
  query-core: a type parameter bound from its first argument alone, later object
  candidates dropped and a fresh object literal never widened
  (`generic-object-candidate-union-basic`); and a `vi.fn()` callback that
  bound nothing, because a rest parameter's element was never lined up and a
  callable object never inferred (`callback-rest-any-inference-basic`). The 2026-09-13 pass closed all eleven in the `react-query`
  type tests, through five root causes: a generic overload group discarding its
  folded parameter union at instantiation, `TS2356` applied to unary `+`/`-`
  where tsc coerces, a declaration's annotation re-checked under the call's
  substitution, a missing-property report made on a literal whose written
  property could not be compared, and — the last one — a call taking the
  overload group's *fold* as its return type rather than the return of the
  overload its arguments actually match (overload return selection, landed the
  same day with flat CPU and memory). Each is pinned by a preset or a Rust test
  and left every other corpus byte-identical. That number was measured on a **dirty
  working tree** on top of `7780246`; re-measure from a clean worktree before
  recording it as a gate. The distribution lives in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md).
- **zod is exact at this commit.** All 21 `tsc` diagnostics are matched at
  file/code/line *and* message text (21/21). The one long-standing over-report
  (`packages/zod/src/v3/types.ts:92:42 TS2339`) was a generic type-predicate
  guard that never narrowed; it is closed. zod remains the most stable perf
  benchmark in the corpus.
- **trpc** is a *measured baseline*, never a parity claim. `tsc` itself reports
  over a thousand diagnostics there (many from examples with unresolved
  workspace imports). The divergence is now one-sided: **0** surge-only, and
  **116** diagnostics `tsc` reports and surge does not, across 106 distinct
  file/code/line locations (led by TS7006 ×37, TS2339 ×29, TS2883 ×10,
  TS18048 ×8). On the 1,128 locations both compilers agree on, message text
  matches **1128 / 1128**. The false-negative side is inventoried by root cause
  in [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-tsc-only-inventory-2026-09-11).
  The surge-only side came down from 65 at `019fb8b` over the 2026-09-07/08
  session; what remained at that commit is listed in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-surge-only-inventory-2026-09-07).
  On 2026-09-10 a **dirty working tree** on top of `6e034fd` measured 1,131
  surge diagnostics, **3** surge-only and 116 `tsc`-only, after closing a
  ten-item TS7006 regression the same tree had introduced and three of the
  twelve inventory items; that figure is provisional and is not carried into
  the table above. The 2026-09-10 inventory is in
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-surge-only-inventory-2026-09-10).

---

## Known limitations

Each entry below carries the date it was last reproduced or refuted against the
oracle. Entries marked **2026-09-03** were re-probed at `f63641d` for this
snapshot; entries marked **2026-09-01** are carried forward from the previous
snapshot without re-verification and are labelled as such.

### Confirmed current gaps

- **An arrow argument's body is inferred by the expression sketch, which
  cannot call a global or instantiate a member generic.** *(reproduced
  2026-09-11 on the dirty tree over `6e034fd`.)* `declare function fn<T
  extends Procedure = Procedure>(impl?: T): Mock<T>` called as
  `fn((value: Date) => value.toISOString())` now binds `T` to the arrow's
  signature on both paths (see
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#closed-t-from-an-arrow-argument-2026-09-11)),
  but `fn((n: number) => String(n))` and `fn(() => p.then(() => "x"))` still
  yield a permissive result: the sketch resolves callees from the local table
  only (`String` is an object with a call signature, not a function symbol)
  and does not instantiate a member's type parameters. `const bad: number =
  fn((n: number) => String(n))` is not reported.
- **Overload resolution: the return type is selected, the no-match diagnostic
  is not.** *(refuted in part 2026-09-13 on `7780246`, dirty tree.)* The
  2026-09-03 example — `declare function f(a: string): string; declare
  function f(a: number): number;` then `const bad: string = f(1)` — now reports
  TS2322 like `tsc`: the arguments are checked once against the group's
  permissive fold and the return type is the first overload's that accepts
  them, at no measured CPU or memory cost (the branch that re-checked every
  candidate, ~+79% CPU, was not rebased). What remains: a call matching *no*
  overload keeps the fold's `TS2345`/`TS2322` where `tsc` reports `TS2769`,
  and interface / type-literal method groups still resolve through the fold
  alone. See
  [REAL_PROJECT_COMPAT.md § Overload return selection](REAL_PROJECT_COMPAT.md#overload-return-selection-2026-09-13).
- **Module augmentation is lost through a star re-export wrapper.**
  *(re-probed 2026-09-03.)* With `declare module "core"` in a `.d.ts` and
  `export * from "core"` in `wrapper`, importing the augmented interface from
  `wrapper` yields the unaugmented shape (a surge-only excess/unknown-property
  error), while importing from `core` directly is correct.
- **An augmented *base* interface is not seen through a derived one.**
  *(reproduced 2026-09-11 on the dirty tree over `6e034fd`.)* With
  `declare module "./generated" { interface BaseNode { parent: Node } }` and
  `interface Identifier extends BaseNode`, `id.parent` is a surge-only TS2339.
  Augmenting `Identifier` itself works — an augmentation merges into the
  per-import export table, while heritage resolves the base in the declaring
  file's own scope, which carries no augmentation. This is what hides `parent`
  on every `@typescript-eslint` AST node. The *relative specifier* half of the
  same gap is fixed — see
  [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#half-closed-a-relative-declare-module-sibling-augmentation-2026-09-11).
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
  though the complete, unmodified lib graph is bundled and loaded.

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
| `SURGE_UPGRADE_ANALYSIS_SCOPES=1` | Give a module's exported declarations the real resolution scope (own declarations + import layers) during analysis instead of the import-less preliminary one. Makes the published type of an export honest where its body names an imported type; on trpc that closes 12 `TS2339` and opens 13, because an honest export lets a conditional decide from a router key set surge never had. See [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md#trpc-tsc-only-inventory-2026-09-11). |
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
A/B** against release binaries built from clean worktrees at the named commits:
six alternating pairs on a quiet machine (load average ~15).

| Metric | `b090760` (prev. snapshot) | `1d5d2b8` (regressed) | `5db229c` (regression closed) |
| --- | ---: | ---: | ---: |
| Peak physical footprint, median | 1.013 GB | 2.011 GB | **1.012 GB** |
| Wall time, median | 5.40 s | 8.98 s | **5.69 s** |
| Diagnostics emitted | 1,190 | 1,153 | **1,153** |

Two false-positive burn-downs have been re-measured against this table since,
both neutral and neither a perf change. `8644a94` against `79cbcfc`, four
interleaved pairs: 1.013 -> 1.012 GB and 4.39 -> 4.46 s on tRPC, 0.516 -> 0.510
GB and 3.40 -> 3.39 s on zod. This snapshot's commit against `8644a94`, four
interleaved pairs: 1.089 -> 1.091 GB and 4.42 -> 4.40 s on tRPC, 0.547 -> 0.547
GB and 3.35 -> 3.33 s on zod. `5689e39` against `8905e69`, which added
JSON-module loading to the import graph: 1.086 -> 1.086 GB and 5.05 -> 5.10 s on
tRPC, 0.547 -> 0.548 GB and 3.80 -> 3.80 s on zod. And this snapshot's commit
against `5689e39`, which resolves a constructor's instance type on every
`instanceof` guard the predicate path sees: 1.087 -> 1.089 GB and 4.21 -> 4.16 s
on tRPC, 0.546 -> 0.547 GB and 3.18 -> 3.17 s on zod. (Absolute wall figures
move between rounds with machine load; only the interleaved pairs within a round
compare.)

**A ~2x memory regression opened and closed inside this snapshot's range.**
`4b3bcc0` reordered the ambient-global collection so that `declare global`
blocks merged first, which made the augmentation the merge base for every
re-opened global interface — and a merged interface takes its declaring file and
resolution scope from whichever fragment merged first. Members then resolved
under the augmenting module's scope: on tRPC, `lib.dom.d.ts::Node` degraded
309,348 times against 727 before, and degraded members are never cached.

Bisected by interleaved measurement:

| Commit | Peak footprint | Wall |
| --- | ---: | ---: |
| `b08adb7` fix(check): narrow Array.filter by a type-predicate callback | 1.011 GB | 5.7–7.0 s |
| `4b3bcc0` fix(program): merge declare-global types before lowering ambient script values | **2.009 GB** | 10.8 s |

`5db229c` splits the ambient pass into types-then-values and merges the
augmentation types between them, which keeps what `4b3bcc0` fixed and returns
both numbers to the `b090760` level. `global-augmentation-merge-base-scope` is
the preset that pins the ordering from both sides.

The diagnostic count moved because that was the point of the 2026-09-07 session:
37 fewer emitted diagnostics on tRPC than at `b090760`, every one of them a
surge-only over-report (see
[§ Real-project compatibility](#real-project-compatibility)).

**Caveats, all of which matter:**

- The local tRPC checkout is `dfbafa8`, **not** the `3e0e979` commit pinned by
  the historical run in [BENCHMARKS.md](BENCHMARKS.md). The workload differs, so
  neither column is comparable to the older 19.7–19.9 s figures.
- Peak RSS on this workload varies ±30–50% run to run *in general*; the tight
  spread here is a property of interleaving, not a guarantee. Memory comparisons
  require interleaved A/B runs in one session. See
  [BENCHMARKS.md § Methodology](BENCHMARKS.md#methodology).
- There is no incremental or persistent mode; every run is a full check.
- The diagnostic surface moved by 37 against `b090760` (1,190 → 1,153), so that
  column is not a like-for-like comparison of identical work. The `1d5d2b8`
  column is: it emits the same 1,153 diagnostics as this commit.

Performance history, methodology, and the reproduction recipe live in
[BENCHMARKS.md](BENCHMARKS.md); the detailed engineering investigations live in
[docs/perf/](docs/perf/) and are point-in-time records, not current state.

---

## What is deliberately not claimed

- **No full TypeScript compatibility claim.** The oracle gate establishes
  parity on the covered fixtures and projects only.
- **No general message-text or span/column parity claim.** The strict sweeps
  are green across the 214 registered presets at this commit bar one message
  drift, which is a statement about those fixtures — not about arbitrary code.
  The strict flags stay non-gating so a preset can record a drift without
  failing CI.
- **No claim that trpc matches `tsc`.** It is a workload and a measured
  baseline; 135 diagnostics still differ in each direction combined.
- **No wall-clock performance claim at this commit** — see the caveat above.
- **No cross-tool performance claim.** `pnpm bench:compilers` exists as a
  developer aid; its output is local-machine-relative and is not a marketing
  comparison.
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
