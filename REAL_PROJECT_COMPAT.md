# Real Project Compatibility

This document measures `surge-ts` (a Rust-based TypeScript noEmit compatibility
checker) against real projects and baseline compilers. `TypeScript`/`tsc` refer
to the upstream compiler used as the oracle baseline. Historical version notes
below may refer to the project by its earlier `surge-ts` / `ts-rust`
labels; those are kept verbatim as measured-at-the-time records. The internal
Cargo crates are still named `surge-ts-*`; the CLI binary is `surge`.

## Current state

> **The canonical current-state summary is [CURRENT_STATUS.md](CURRENT_STATUS.md).**
> This document is the detailed measurement record behind it: per-project
> baselines, drift categories, burn-down history, and the known-limitation
> reproductions. Where a number appears in both places, `CURRENT_STATUS.md` is
> the one that gets re-measured.

The per-project counts are volatile, so they live in
[CURRENT_STATUS.md § Real-project compatibility](CURRENT_STATUS.md#real-project-compatibility)
and nowhere else; that table is the one that gets re-measured. What lives here
is the detail behind it: the per-project drift inventories below, the
burn-down history, and the known-limitation reproductions.

- The oracle preset sweep is green under the normal gate (diagnostic code-count
  and file/code/line). Message-text and span/column drift are reported but
  non-gating unless `--strictMessages` / `--strictSpans` are passed (see
  [STRICT_DRIFT_INVENTORY.md](STRICT_DRIFT_INVENTORY.md)). The preset count and
  the current strict state are recorded in
  [CURRENT_STATUS.md § Gates](CURRENT_STATUS.md#gates).
- The compact `diagnostics-pack` preset is green at exact 31/31 parity under
  the normal gate *and* both strict gates. It pins duplicate declaration /
  function-implementation parity (TS2451/TS2393 on every conflicting
  declaration), the TDZ TS2448+TS2454 pairing for block-scoped reads in the
  temporal dead zone, missing-return span placement (TS2355/TS2366 on the
  function/method name span), and use-site generic-arity spans (TS2314/TS2315).
  This is targeted emitted-diagnostic parity, not full TypeScript parity.
- Project mode loads the physical `lib*.d.ts` graph by default; the generated
  default-lib subset is a fallback when the `typescript` package is absent, not
  the normal project-mode source of truth. `noLib: true` keeps standard/DOM
  globals unavailable.
- Performance has been stabilized by a program-wide generic instantiation cache
  for context-free library/dependency declarations and by deferring the
  interface/alias payload clone in named-type resolution to a genuine cache
  miss. Regression fixtures (`generic-cache-dependency-instantiation-basic`,
  `generic-cache-module-source-not-persisted-basic`,
  `generic-cache-unresolved-argument-diagnostics-basic`) are registered as
  oracle presets.
- **Do not read wall-clock figures out of the version-tagged notes.** The
  `~0.20s` auth-kit medians in the historical `v1.2.5` notes predate
  physical-lib-by-default; the auth-kit numbers in the `v0.86`–`v0.92` notes
  predate it too. Current benchmark figures live in
  [CURRENT_STATUS.md](CURRENT_STATUS.md#current-performance-state) and
  [BENCHMARKS.md](BENCHMARKS.md); everything older is in
  [docs/history/](docs/history/) and [docs/perf/](docs/perf/) as a
  point-in-time record.

## trpc surge-only inventory (2026-09-07)

Measured at commit `8644a94` against the pinned TypeScript 7.0.2 oracle, tRPC
checkout `dfbafa8`. `tsc` reports 1,244 diagnostics there and surge-ts 1,141;
this section lists only the **surge-only** side — locations where surge reports
something `tsc` does not. It is a burn-down list, not a parity claim: the
`tsc`-only side (123 at this commit) is tracked separately, and neither side is
gated.

The surge-only side was 65 at `019fb8b` and is 20 here. What closed, and the
oracle preset that pins each, is in the commit range `019fb8b..8644a94`:
`this-type-predicate-narrowing-basic`, `guard-polarity-narrowing-basic`,
`overload-merge-contextual-callback-basic`, `namespace-callback-parameter-basic`,
`exit-and-alias-narrowing-basic`, `generator-missing-return-basic`,
`assertion-and-nonnullable-basic`, `class-prototype-instanceof-basic`,
`tuple-union-destructure-basic`, `intersection-two-union-operands-basic`, plus
the local-shadow case added to `umd-global-module-reference-basic`. Two of the
fixes are not preset-pinned because their trigger is a shape surge fails to
model and `tsc` types fine — a fixture would pin the modelling gap rather than
the suppression; both are called out in their commits.

The 20 that remain, with the root cause where it is known:

| Location | Code | Root cause |
| --- | --- | --- |
| `examples/next-sse-chat/src/server/db/schema.ts:13`, `:61` ×2 | TS4111 | drizzle's `pgTable(...)` result degrades to a string index signature, so column access reads as index-signature access under `noPropertyAccessFromIndexSignature`. Only reproduces inside the full project — the same file checked on its own is clean. |
| `packages/server/src/__tests__/trpcServerResource.ts:53`, `:64` | TS7006, TS2349 | `createHTTPHandler`'s options are an intersection whose `CreateContextCallback<…>` operand is a conditional over an unresolved router context. Replacing that operand with a plain object closes `:53`, so the contextual type is lost through the operand — but the object-literal path that loses it has not been isolated. |
| `packages/server/src/unstable-core-do-not-import/router.ts:75` | TS2314 | `DecorateRouterRecord<TRecord>` resolves to the two-parameter declaration in `packages/react-query`, a cross-module type-name leak. Not reduced. |
| `packages/upgrade/src/bin/index.ts:5` | TS2307 | `import { version } from '../../package.json'`. `.json` specifiers are pinned unsupported in the relative-specifier classifier; `resolveJsonModule` is parsed but not implemented. |
| `packages/server/src/observable/observable.test.ts:1` | TS2305 | `import { EventEmitter } from 'stream'` — `@types/node`'s `stream` module imports `EventEmitter` without re-exporting it, so the resolution `tsc` uses here is not the one surge takes. |
| `examples/nuxt/nuxt.config.ts:2` | TS2304 | `defineNuxtConfig` comes from Nuxt's generated `.nuxt` types. |
| `packages/client/src/links/loggerLink.ts:204` | TS2339 | `props.result instanceof Error || ('error' in props.result.result && …)`: the right operand of an `||` needs the left's *falsity* to narrow a property path by `instanceof`, and the name-based member test cannot see that `TRPCClientError extends Error`. The union-aware fallback exists for identifiers, not for reference paths — that path has no `ctx` to resolve the constructor's instance type. |
| the remaining 10 | TS2322 ×4, TS2345 ×2, TS2339 ×2, TS2554, TS2353 | one-off assignability, arity and member divergences inside tRPC's generic builder machinery; not yet reduced |

## Compatibility fixture matrix

Fourteen real-world-shaped oracle presets (added 2026-07-16) pin the library
patterns below at exact diagnostic parity (code-count and file/code/line)
against the native tsc oracle. Each fixture is registered in
`scripts/oracle/compare-tsc.ts` and covered by
`pnpm run oracle:sweep -- --all`. "Pos" counts the typed positive assertions
that must stay diagnostic-free; "neg" counts intentional errors pinned at exact
file/code/line parity.

These fourteen are a slice of the registry, not the whole of it — the total
preset count and its last verified sweep result are in
[CURRENT_STATUS.md](CURRENT_STATUS.md#gates).

| Fixture | Family | What it pins | Pos/Neg |
| --- | --- | --- | --- |
| `react19-jsx-function-component-basic` | React 19 / JSX | `jsx: react-jsx` runtime lookup through `@types/react` `React.JSX`, intrinsic elements, function-component props, `children`, ref-as-prop, callback contextual typing inside JSX props | 5/1 |
| `react19-jsx-generic-component-basic` | React 19 / JSX | generic function components: inference from props, explicit `<List<string>>` type arguments, namespace import of React, callback param inference in JSX attributes | 3/1 |
| `query-generics-observer-basic` | TanStack-style generics | explicit-type-arg generic hook over an options object, optional contextual callbacks (`onSuccess`/`select`), Promise-returning `queryFn`, alias-annotated result, optional chaining on `data`/`error` | 5/2 |
| `query-generics-options-mapped-basic` | TanStack-style generics | mapped result record over an inferred constrained record (indexed access in the mapped body), nested generic aliases, top-level conditional `infer` alias with concrete args | 5/1 |
| `schema-inference-nested-basic` | Zod-style inference | `_output` indexed-access inference through nested `object`/`array`/`optional`/`union` combinators, `Infer` alias round-trip | 7/1 |
| `schema-inference-recursive-basic` | Zod-style inference | recursive interface schema via `lazy<T>` with explicit type arg and self-referencing const, deep recursive member chains | 2/1 |
| `express-augmentation-cycle-basic` | Express-style augmentation + cycles | cyclic `.d.ts` imports (`index` <-> `application`), module augmentation of a package interface from a second package and from a consumer `.d.ts`, merged members visible on direct import | 5/1 |
| `express-augmentation-cycle-collision-pinned` | Express-style augmentation + cycles | consumer-local `Store` types (interface in one file, alias in another) colliding with the dependency-internal `Store`; dependency-scope name resolution inside dependency interfaces, cycle re-entry (`store.connection().store`) — pins per-environment resolution against naive cross-consumer caching | 4/2 |
| `router-graph-procedures-basic` | tRPC-style router graph | nested router records (intersection + record inference), callable procedure interfaces, conditional `InputOf`/`OutputOf` extraction over concrete `typeof`, mapped record over a router, Promise-typed results, void-input call | 7/1 |
| `router-graph-subscription-basic` | tRPC-style router graph | nested generic interfaces (`Subscription<Envelope<T>>`), contextual callback parameter typing from declared function types, generic member chains | 5/1 |
| `node-decl-callable-namespace-basic` | Node declaration shapes | `typeRoots` + `types`-configured `@types` packages, `export =` callable import via `import ... = require(...)`, namespace type member access (`log.Options`), ambient `declare var` global, ambient namespace const/interface | 5/1 |
| `node-decl-subpath-cts-mts-basic` | Node declaration shapes | package.json `exports` `types` conditions, subpath exports, `import`/`require` condition split resolving the `.d.mts` surface | 4/1 |
| `combined-conditional-mapped-indexed-basic` | Combined features | conditional + mapped + indexed-access combinations, distributive conditional over a union, recursive generic interface substitution (`Tree<T>` flatten inference), constrained mapped `PickByKey` | 8/1 |
| `combined-augmentation-generic-registry-basic` | Combined features | module augmentation adding a member to a generic interface and adding a module export, explicit generic annotation resolving augmented members, imported generic type usage | 5/1 |

### Known limitations discovered (excluded from the fixtures above)

> **Status: partially superseded (re-checked 2026-09-01 at commit `37dfb3a`).**
> This list was written between 2026-07-16 and 2026-07-29. Several entries have
> since been fixed and are marked **RESOLVED** below, with their investigation
> history kept because it explains *how* the fix was reached (and which
> approaches were measured and rejected). The current, probe-verified
> limitation list is
> [CURRENT_STATUS.md § Known limitations](CURRENT_STATUS.md#known-limitations);
> read that first and treat this section as the detailed record behind it.

Shapes below were reduced out of the fixtures rather than pinned. Each was a
candidate checker fix; none is gated.

- **RESOLVED (verified 2026-09-01).** Qualified heritage clauses now resolve:
  `tests/compat-projects/interface-qualified-heritage-basic` measures **8 tsc /
  8 surge, file/code/line and message text all matching**. The fixture is still
  *not registered* as an oracle preset — registering it is open follow-up work.
  The original problem statement and the three rejected performance attempts
  are kept below because they document the cost model that any similar change
  has to clear.

  A qualified heritage clause (`interface X extends NS.Member`,
  `class C extends NS.Base`) inherits **no** members at all — not merely the
  cross-file merged ones. `parse_interface_heritage` /`parse_class_heritage`
  (`crates/surge-ts-syntax/src/parser/`) accept only a bare
  `Expression::Identifier`, so a dotted base is dropped at parse time and the
  derived type is built with an empty heritage list. Every inherited access is
  then a surge-only TS2339, and tsc's TS2503/TS2694 for an unresolvable
  qualified base are never emitted. This affects local, `declare`, global, and
  namespace-imported namespaces alike; direct type-position use of
  `NS.Member` is correct. This is the `Express.Request` pattern; the express
  fixtures use `declare module` augmentation instead.

  Reproduction (measured 2026-07-29, tsc 7.0.2):
  `tests/compat-projects/interface-qualified-heritage-basic` — 8 intentional
  errors, ~24 positive assertions. It is **not registered** as an oracle
  preset because the checker does not support the shape; see the fixture's
  `README.md`. A parse-side fix was implemented and measured this session: it
  reached exact 8/8 parity there and removed 371 net zod false positives, but
  regressed zod by +77% wall / +44% peak footprint and tRPC by +10% / +7%, so
  it was reverted under the Phase 8 policy. Root cause of the regression:
  resolving the previously-dropped bases roughly doubles
  `interface_resolution_attempt_count` and more than doubles
  `interface_resolution_degraded_count` (zod 61k → 147k, tRPC 69k → 126k);
  degraded resolutions are never cached (see
  [docs/PERFORMANCE_INVARIANTS.md](docs/PERFORMANCE_INVARIANTS.md)), so each
  one is recomputed at every use site. Landing this shape needs the
  underlying base resolutions (`stream.Writable`, `http.IncomingMessage`,
  and nested-namespace-through-import members) to resolve *cleanly* first, so
  the results become cacheable.
  Fixing the qualified-member resolution below did **not** unblock it: with
  both changes applied, zod's `interface_resolution_degraded_count` is
  unchanged at 147,148. The trace instead shows `cache_hits=0` on every hot
  user generic (`$ZodTypeInternals`: 31,121 attempts, 150 unique argument
  tuples, 0 cache hits; `lib.es2015.collection.d.ts::ReadonlyMap`: 9,474
  attempts, 100% degraded).

  Re-measured 2026-07-29 after the signature-context generic-instantiation
  cache landed (see
  [docs/perf/SIGNATURE-CONTEXT-GENERIC-CACHE.md](docs/perf/SIGNATURE-CONTEXT-GENERIC-CACHE.md)):
  still rejected. The cache doubles its zod hits under the patch
  (7,001 → 13,110) but only absorbs the *clean* fraction; the explosion is in
  degraded resolutions (zod 60,961 → 147,132, attempts 162,686 → 292,888),
  which are uncacheable by invariant — and demonstrably so: an experiment
  that program-wide-cached the sentinel-embedding expansions changed real zod
  diagnostics (four TS2339s vanished, one message render drifted). Interleaved
  A/B at the patch (7/7 pairs): zod `--jobs auto` 2.13 s → 3.78 s (+77%),
  peak footprint 645 MB → 885 MB (+37%). The remaining lever is making the
  newly-resolved base expansions *clean* (they degrade because their bodies
  carry placeholder/`unknown`-sentinel members in generic contexts), not
  caching them.

  Degradation-provenance work, 2026-07-29 (see
  [docs/perf/CLEAN-GENERIC-BASE-EXPANSION.md](docs/perf/CLEAN-GENERIC-BASE-EXPANSION.md)):
  a per-origin `had_error` trace split the explosion into (1) a check-phase
  unbound-`infer`-capture family (`MakeReadonly`'s `Map<infer K, infer V>`
  branch selected for `any`/sentinel members, then `ReadonlyMap<K, V>`
  resolved with `K`/`V` unresolvable) — fixed by the distributive-conditional
  member guards, removing 3 zod + 6 tRPC false positives with zero additions —
  and (2) a larger analysis-phase family where declaration bodies resolve
  under import-less pre-attached scopes while `module_scope_by_file` is
  deliberately absent (the `core.output`-class silent misses). Repairing (2)
  via authoritative-map substitution in environment recovery was measured and
  **reverted**: it exposed a latent two-copy nominal-identity gap in
  dependency `.d.ts` resolution (three new FPs: tinybench `Task` vs `Task`,
  `QueryClient` vs `QueryClient`, playwright `MakeMatchers`). That identity
  gap is the prerequisite for the next attempt on (2), which is itself the
  bulk of the heritage explosion. The guards landed with zod 912 → 909 and
  tRPC 1878 → 1872 (all removals, adjudicated FP-only; ky/ofetch/unnamed
  byte-identical) and neutral-to-positive perf (zod auto −1.7% wall, 11/11
  pairs). Heritage re-applied on top of them was re-measured: zod auto
  2.27 s → 3.61 s (**+59%**, 0/7 pairs) / peak +26% — better than the
  pre-batch +77%/+37% but still rejected; degraded resolutions under
  heritage fall 147,132 → 126,517.
- ~~A namespace member reached through a namespace import with three or more
  segments (`import * as N from "./m"; N.NS.Member`) does not resolve.~~
  **Fixed 2026-07-29** and gated by the
  `namespace-import-qualified-member-basic` preset. The namespace-import alias
  table flattened a qualified export key (`util.TupleItems`) to its last
  segment, registering `core.TupleItems` instead of `core.util.TupleItems`, so
  the real name never resolved and the type silently opened (a false negative:
  tsc's member errors were missed). The alias table now keys members by their
  full exported path. Named-namespace imports (`import { util }`) were already
  correct. Note the residual message-only drift: surge renders the qualified
  name (`core.util.TupleItems`) where tsc renders the declaration name
  (`TupleItems`); this is pre-existing, non-gating, and applies to all
  qualified references.
- **CONFIRMED (re-probed 2026-09-01).** Module augmentations of a package
  interface are lost when the interface is imported through a star re-export
  wrapper (`export * from "core"` in `wrapper`; importing from `wrapper`
  yields the unaugmented shape and a surge-only TS2339, importing from `core`
  directly is 0/0 correct). Fixtures import from the core package directly.
- **Split verdict (re-probed 2026-09-01).**
  - **RESOLVED:** generic type-parameter inference from a callback argument's
    return type (`fn: () => T`, `fn: () => Promise<T>`) now infers and reports
    like tsc, at matching file/code/line. One message-text drift remains on the
    `Promise<T>` form.
  - **CONFIRMED:** generic JSX components still miss prop mismatches on
    type-parameter-dependent props (explicit `<List<string>>` or inference
    conflicts); non-generic props on generic components are checked.
    Query/schema fixtures pin explicit-type-arg and value-inference paths
    instead.
- **RESOLVED (re-probed 2026-09-01).** Conditional types with `infer` inside a
  mapped-type body, and with the infer position inside an object-literal type
  (`{ initial: infer V }`), no longer produce a false TS2304; a probe of both
  shapes matches tsc exactly, message text included. Historically both raised
  "Cannot find name"; top-level conditional-infer aliases over
  interface/alias/`Promise` references with concrete arguments always worked.
- **RESOLVED (re-probed 2026-09-01).** A required property typed
  `string | undefined` against an optional target property (`slug?: string`)
  inside a generic comparison now measures 0/0 against tsc. It historically
  produced a surge-only TS2322 where tsc accepts without
  `exactOptionalPropertyTypes`.
- **RESOLVED (re-probed 2026-09-01).** `async function f(): Promise<void> {}`
  with no return statement no longer raises a false TS2355; a probe covering
  both `Promise<void>` and `Promise<number>` matches tsc exactly.
- **Downgraded to display-only (re-probed 2026-09-01).** Module-scope bindings
  holding a generic-instantiated callable interface
  (`const p = procedure<I, O>()` imported cross-module) no longer surface
  TS2339 false positives when called inside later function bodies or chained
  (`.then`): a probe of both shapes matches tsc at code-count and
  file/code/line. What remains is a **message-text** gap — surge renders the
  uninstantiated `Promise<O>` where tsc renders the substituted `number`. The
  augmentation-member half of this entry (unannotated module-scope
  `createRegistry<string>()` missing augmentation-added members) was not
  re-probed for this snapshot.
- **CONFIRMED, manifestation changed (re-probed 2026-09-01).** Overload groups
  (function declarations and interface call signatures) still resolve against
  the first overload only. It now surfaces as an **under-report** rather than a
  false TS2345: `f(1)` against `f(a: string): string` / `f(a: number): number`
  is typed from the first signature, so tsc's TS2322 on the assignment is
  missed entirely. The subscription fixture uses distinct functions instead of
  overloads. A full overload-resolution implementation exists on a branch but
  is blocked on a measured ~+79% CPU regression.
- **CONFIRMED (re-probed 2026-09-01).** Callable-function + namespace
  declaration merging is still inconsistent. In a same-file merge the
  callability is dropped: `declare function log(msg: string): void;` merged
  with `declare namespace log { … }` makes `log("hi")` a false TS2349 ("This
  expression is not callable"), and tsc's genuine TS2322 on a mistyped
  `log.version` read is missed. The `export =` direction is covered by the
  `node-decl-callable-namespace-basic` preset, which is green.
- **CONFIRMED (re-probed 2026-09-01).** `typeof import("pkg")` in a type alias
  does not resolve (surge-only TS2304 at the alias use site). Dynamic
  `import()` and `import("m").T` are parser gaps as well — see
  [crates/surge-ts/MODULE_RESOLUTION.md](crates/surge-ts/MODULE_RESOLUTION.md).

## unnamed (Next.js real-project measurement)

`unnamed` is the second real-project compatibility target after auth-kit: a local
Next.js App-Router project (`moduleResolution: bundler`, `jsx: react-jsx`,
`strict`, `paths: { "@/*": ["./*"] }`, `lib: dom/dom.iterable/esnext`, includes
`.next/types/**`). It is **not** a parity claim — it is a measured baseline used
to find the highest-impact compatibility blocker. Full Next.js / React / Prisma
parity remains out of scope.

- Command:
  `pnpm run real:unnamed`
  (`scripts/real-projects/measure-project.ts --project ../../nextjs/unnamed
  --name unnamed --allowMissing`), plus
  `pnpm run oracle:compare -- --project ../../nextjs/unnamed/tsconfig.json
  --maxDiagnostics 200`.
- Local project present: **yes** (`../../nextjs/unnamed`). Project source is never
  copied into this repo; `--allowMissing` keeps the script honest when absent.
- Artifacts: `.bench/real-projects/unnamed/` (`oracle-compare.json`,
  `compat-report.json`, `timings.txt`, `measurement.md`).

### Current measurement (2026-09-01, commit `37dfb3a`): 0/0, exact tsc parity

`unnamed` matches tsc exactly: **0 diagnostics on both sides**, no false
positives and no false negatives. Re-verified 2026-09-01; first reached
2026-08-20. It is pinned as a strict false-positive regression gate by
`pnpm run real:unnamed:test`, which skips cleanly when the project is absent.
The subsections below are the historical burn-down and are kept verbatim as
measured-at-the-time records.

The last false positive was a `TS7006` on a `useState` updater callback, and it
was the visible tip of a much larger hole: `export = <declare namespace>` exposed
only the namespace object, whose members are modelled permissively, so **every
React hook call lost its return type**. Closing it needed three changes together
— accepting a signature that names a type the namespace *exports*, carrying a
`namespace_prefix` on the signature so instantiation re-resolves sibling names
under it, and carrying the qualified value members through `export =`. See the
`fix(check): carry a namespace member's real signature to its callers` commit.

### Measured baseline

| Metric | Value |
| --- | ---: |
| TypeScript (tsc) diagnostics | 0 |
| surge-ts diagnostics (before this pass) | 259 |
| surge-ts diagnostics (after this pass) | 230 |
| raw oracle match | no (surge-ts over-reports; every surge-ts diagnostic is a false positive) |

tsc reports a clean `0`, so all surge-ts diagnostics are over-reports. This is an
honest "not close to parity" baseline, expected for a real Next.js app exercising
React/JSX contextual typing, generated route types, and namespaces — all
currently out of scope.

### Drift categories (surge-only, after this pass; tsc = 0)

| Code | Count | Category |
| --- | ---: | --- |
| TS7031 | 57 | JSX/React contextual callback param typing (implicit-any binding elements, e.g. `render={({ field }) => …}`) |
| TS2339 | 49 | property access on narrowed/union and unmodelled-lib receivers |
| TS7006 | 23 | JSX/React contextual callback param typing (implicit-any params, e.g. `onCheckedChange={(checked) => …}`) |
| TS2304 | 21 | namespaces / generated globals (`Prisma`, Next generated `Display`/`NextFontWithVariable`) |
| TS2536 | 19 | generated Next.js route types (`.next/types/validator.ts` `ParamMap` indexing) / namespace index access |
| TS2345 | 17 | string-literal-union argument widening/narrowing (`as const` lookup tables) |
| TS2305 | 11 | type-only re-exports of namespace values (`import type { z }`) and exported-type-with-unresolved-RHS (`Locale`) |
| TS2322 | 10 | assignability after narrowing |
| TS2349 | 10 | not-callable on unmodelled shapes |
| TS2741 | 6 | missing required property |
| TS2538 / TS2314 / TS2882 | 4 / 2 / 1 | misc index/generic-arity/side-effect-import |

Dominant blockers (TS7031 + TS7006 = 80, ~35% of drift) are React/JSX contextual
callback inference — explicitly out of scope for this pass (would require broad
contextual-typing/generic-inference work). Generated-route-type (TS2536) and
namespace (TS2304/`Prisma`) drift are also out of scope.

### Blocker selected and fixed

**Re-export of an imported binding** (`import * as z from "zod"; export { z }`).
This was the root cause of the zod `z` over-reports: every form file does
`import { z } from "zod"`, and zod's `index.d.cts` is
`import * as z from "./v4/classic/external.cjs"; export { z }`. surge-ts resolved
the namespace import for *expression* use (`z.object(...)` worked) but the
`export { z }` named-re-export path did not recognize a namespace/named/default
**import** binding as a valid local export source, so it emitted `TS2304 Cannot
find name 'z'` on the re-export and `TS2305 … has no exported member 'z'` on every
consumer. tsc accepts all three import forms re-exported by name.

Fix (smallest reproduction: `tests/compat-projects/namespace-import-reexport-basic`,
a relative-module shape derived from the drift category, not copied from
`unnamed`): the final module export-table build now threads the file's resolved
import symbols (`ModuleImportBindings.symbols`) into the `export { name }`
re-export lookup, so a re-exported namespace/named/default import resolves to its
real imported binding. The import symbols are used **only** for the named
re-export fallback — not the general initializer-inference environment — so
`export const X = ns.member()` initializer inference is unchanged (an earlier
broader version surfaced one cascade false positive on
`new $Class.getPrismaClientClass()(...)`; scoping the change to the re-export
lookup removed it). Value re-exports now resolve with their precise type, not a
fallback-to-`any`.

Impact on `unnamed`: 259 → 230 surge-only diagnostics (−29, the zod `z` cascade:
9 direct TS2305 plus ~18 downstream TS2339 and misc), **zero new false positives**.
auth-kit stays exact `0/0`; the oracle preset sweep is **76/76** (75 prior + the
new fixture) under the normal gate.

### Remaining next recommended fix

Type-only re-export of a namespace value (`import type { z } from "zod"` in the 3
`*-form.tsx` files, plus the exported-type `Locale` whose RHS
`(typeof routing.locales)[number]` is unresolved) — the type-side analogue of the
value re-export fixed here. After that, the dominant remaining drift
(TS7031/TS7006 React contextual callback inference) is the next high-impact but
much larger area; it should not be attempted as a "small blocker".

The version-tagged milestone notes that used to follow here — the `v0.60`
instrumentation baseline through the `v0.93`–`v1.2.5` auth-kit optimization
log — have moved to
[docs/history/REAL_PROJECT_COMPAT-HISTORY.md](docs/history/REAL_PROJECT_COMPAT-HISTORY.md).
They record how the checker reached this state; their wall-clock medians and
their "synthetic built-ins" / "generated default-lib" descriptions reflect the
measurement and lib model in effect at the time, **not** current behavior.

## ky (Fetch-API real-project parity)

`ky` is [sindresorhus/ky](https://github.com/sindresorhus/ky) 2.0.2: ~29 small
Fetch-API / DOM-typed source files, `tsconfig` extending `@sindresorhus/tsconfig`
(lib `DOM`+`DOM.Iterable`+`ES2023`, `exactOptionalPropertyTypes`, target
`esnext`). **`tsc` reports 0 diagnostics on it**, so it is a strict
false-positive corpus: every surge-ts diagnostic is a known-wrong over-report.
Unlike `unnamed` (a measured baseline), ky is now used as a **parity claim and a
regression gate**.

- Command: `pnpm run real:ky`
  (`measure-project.ts --project .local-projects/ky --name ky --allowMissing`),
  plus `pnpm run oracle:compare -- --project .local-projects/ky/tsconfig.json
  --failOnMismatch`.
- Local project present: gated. `.local-projects/` is gitignored — ky is **not
  vendored**. The source is never copied into this repo; `--allowMissing` keeps
  the script honest when absent.
- Artifacts: `.bench/real-projects/ky/` (`measurement.md`, `compat-report.json`).

### Current measurement (2026-09-01, commit `37dfb3a`, checkout `3419113`): 0/0

Re-verified 2026-09-01; first reached 2026-06-20.

- TypeScript total diagnostics: **0**. surge-ts total diagnostics: **0**.
- code-count match: **yes**; file/code match: **yes**; only-TypeScript: 0;
  only-surge-ts: 0. surge-ts matches tsc exactly.
- Regression gate: `pnpm run real:ky:test` (also run by `pnpm run real:test`)
  runs the surge↔tsc comparison and fails on any drift from 0/0. It **skips**
  when ky or the `typescript` package is absent (mirroring the physical-lib rust
  tests). The specific patterns that were fixed are additionally pinned as
  cargo fixtures: `tests/compat-projects/physical-lib-new-promise-executor-basic`
  and `tests/compat-projects/physical-lib-required-omit-pick-basic`, plus the
  `cli_*` regressions in `crates/surge-ts-cli/tests/project_mode.rs`.

### Suppression / stub transparency (not yet audited)

Source-level parity is 0/0, but the compatReport shows three non-zero suppression
counters on ky that gate the parity claim and need a transparent audit (tracked
in `crates/surge-ts-checker/SUPPRESSED_DIAGNOSTICS_AUDIT.md`):

- `suppressedRustOnlyDiagnosticsTotal = 15` — `surge::*` diagnostics
  (parser/internal limits, never TS codes) suppressed before user output.
- `suppressedDeclarationDiagnosticsTotal = 23` — diagnostics inside declaration
  (`.d.ts`) files suppressed (trusted upstream lib/dependency declarations).
- `externalModuleStubs.total = 1` — one imported module resolved to a stub
  rather than a real declaration.

These do not affect the source-file comparison, but a product-grade "matches
tsc on ky" claim must confirm none of them hides a real source-level miss.

### History (false-positive burn-down)

ky was adopted mid-2026 as a false-positive corpus. The over-report count fell
across a sequence of targeted checker fixes (each verified oracle-clean):
**~42 (post-runaway) → 39 → 36 → 22 → 16 → 13 → 6 → 3 → 2 → 0**. The
`36`-remaining and intermediate states are kept here as historical records of
that burn-down; they were real measured over-report counts at the time, not the
current parity. The final clusters cleared on 2026-06-20 were: a `typeof
<importedValue>` module-value fallback, an `any`-typed callee being callable,
OR-of-guards + `ArrayBuffer.isView` narrowing, contextual `new Promise<void>`
generic-constructor inference (+ function→`Function` assignability), and the
`Required`/`Readonly` utility resolution (+ generic-context TS2538 suppression
and `&&`-chain truthy-property narrowing). The detailed root-cause taxonomy
lives in the working notes, not this doc.

## ofetch (Fetch-API real-project measurement)

`ofetch` is [unjs/ofetch](https://github.com/unjs/ofetch): a small Fetch-API
wrapper, 7 source files under `src/` plus a `test/` suite, `tsconfig` with
`module`/`moduleResolution: NodeNext`, `strict`, `verbatimModuleSyntax`,
`isolatedModules`, `isolatedDeclarations`, `composite`. **`tsc` reports 1
diagnostic** — a removed-compiler-option report on `esModuleInterop=false` —
so it is a near false-positive corpus rather than a strict 0-baseline.

### Current measurement (2026-09-01, commit `37dfb3a`, checkout `1dbc37f`): exact

`tsc` 1 / surge-ts 1, matching at code-count, file/code/line **and** message
text: `tsconfig.json(7,…) TS5108: Option 'esModuleInterop=false' has been
removed.` surge models removed-option reporting (`TS5102`/`TS5108`) in the CLI,
so this is a match rather than an under-report.

The subsections below are the historical burn-down and are kept verbatim as
measured-at-the-time records. Note that the pinned oracle reports this
diagnostic as `TS5108` under TypeScript 7.0.2; the older `TS5107` spelling in
those notes is what the then-current oracle produced.

- Command: `pnpm run real:ofetch`
  (`measure-project.ts --project .local-projects/ofetch --name ofetch
  --allowMissing`), plus `pnpm run oracle:compare -- --project
  .local-projects/ofetch/tsconfig.json --maxDiagnostics 300`.
- Local project present: gated. `.local-projects/` is gitignored — ofetch is
  **not vendored**; `--allowMissing` keeps the script honest when absent.

### Measured baseline

| Metric | Before this pass | After this pass |
| --- | ---: | ---: |
| TypeScript (tsc) diagnostics | 1 (`TS5107`) | 1 (`TS5107`) |
| surge-ts diagnostics | 5 | 2 |
| surge-ts over-reports (false positives) | 5 | 2 |

tsc's single diagnostic is the `esModuleInterop=false` deprecation
(`TS5107`), which surge does not model (no compiler-option deprecation
diagnostics exist yet) — an under-report, not a false positive.

### False positives fixed this pass (5 → 2)

Three checker over-reports were root-caused and fixed (each verified against the
oracle preset sweep, still green):

- **`TS2339` on primitive literal index access** — `path[0]` (with `path:
  string`) reported `Property 'path' does not exist on type 'string'`. The
  statement-level index-access evaluator emitted a missing-property error for any
  literal index on any receiver, naming the *receiver* as the absent property.
  Restricted to object-like receivers (`Object`/`Function`/`Reference`):
  primitives carry an apparent type with index signatures (`string[number] ->
  string`), so a literal index there is never a `TS2339`.
  ([`checks/expr/`](crates/surge-ts-checker/src/checks/expr/))
- **`TS2349` "not callable" after truthy narrowing** — `if (hooks) { hooks(ctx) }`
  with `hooks: Hook | undefined` reported the call as not callable. The
  positive-branch narrowing handled `typeof`/`instanceof`/`Array.isArray`/
  discriminant guards but not a bare-identifier truthy guard, so `undefined` was
  never dropped and the callee stayed `Hook | undefined`. Added bare-identifier
  truthy narrowing (`remove_nullish` on the true branch); the existing `!guard`
  unwrap routes `if (!x)` else/fall-through through the same path.
  ([`checks/function/narrowing/`](crates/surge-ts-checker/src/checks/function/narrowing/))
- **`TS2339` for `Object.prototype` members on object/named types** —
  `error.toString()` reported `Property 'toString' does not exist on type
  'Error'`. Object and named-interface apparent types now expose the
  `Object.prototype` members.
  ([`types/object.rs`](crates/surge-ts-types/src/object.rs))

### Historical: the two `node:*` over-reports (RESOLVED)

**Resolved.** Transitive `/// <reference types="…" />` loading from dependency
declaration files — described below as "the faithful fix … tracked as future
work" — is implemented
(`ReferenceTypeDirectiveResolver` in `crates/surge-ts/src/package_declarations/`
follows a loaded type package's own directives recursively). ofetch measures
exact at the current commit. The analysis below is kept as the record of how
the gap was diagnosed and which heuristic was rejected.

#### Original entry: `node:*` import resolution via transitively-loaded `@types/node`

Both remaining over-reports are `TS2591` on `import … from "node:stream"`
(`src/fetch.ts`, `test/index.test.ts`). tsc resolves these because `@types/node`
is **loaded transitively** — a dependency `.d.ts` (vitest/vite) carries a `///
<reference types="node" />` that pulls the package into the program, which makes
its `declare module "node:stream"` visible. surge does not follow that transitive
type-reference chain, so the specifier stays unresolved and surge emits the
install hint.

A stub-resolution heuristic ("treat Node-core imports as resolved when an
`@types/node` package exists on disk") was prototyped and **rejected**: tsc does
*not* resolve `node:*` from an on-disk `@types/node` alone (with `types` absent or
`types: []`, a minimal project still reports `TS2591`), and the heuristic
regressed `node-protocol-no-node-types-basic` by suppressing two real `TS2591`s
it picked up from the repo-root `node_modules/@types/node`. The faithful fix is
transitive `/// <reference types="..." />` loading from dependency declaration
files, tracked as future work; full Node/`@types` resolution parity stays out of
scope.

## zod (schema-library real-project measurement)

`zod` (`.local-projects/zod`) is the most-used *performance* corpus in this
repository — it is the stable benchmark referenced throughout
[docs/perf/](docs/perf/) — and it doubles as a compatibility target. It had no
section here until 2026-09-01; the engineering notes elsewhere in this document
reference its diagnostic counts, so this section is where its measured state
belongs.

- Command:
  `pnpm run oracle:compare -- --project .local-projects/zod/tsconfig.json --maxDiagnostics 500`
  (measurement harness: `pnpm run real:zod`).
- Local project present: gated. `.local-projects/` is gitignored — zod is
  **not vendored**.

### Current measurement (2026-09-01, commit `37dfb3a`, checkout `912f0f5`)

| Metric | Value |
| --- | ---: |
| TypeScript (tsc) diagnostics | 21 |
| surge-ts diagnostics | 22 |
| matched | 21 (all of tsc's, message text included) |
| surge-only (over-report) | 1 |
| tsc-only (under-report) | 0 |

The single over-report is
`packages/zod/src/v3/types.ts:92:42 TS2339 Property 'value' does not exist on
type 'INVALID'.` Everything tsc reports is matched at file/code/line **and**
message text.

zod is **not** currently pinned as a regression gate (there is no
`real:zod:test`), because it is not at parity. Treat the one-diagnostic gap as
open work, not as noise: the perf reports in [docs/perf/](docs/perf/) routinely
use zod diagnostic counts as the correctness control for an optimization, so a
drift here invalidates those comparisons.

Historical note: several perf reports quote much larger zod surge-only counts
(for example "zod 912 → 909" during the 2026-07-29 heritage investigation, or
"494 surge-only, capped"). Those are measured-at-the-time figures from before
the 2026-08 false-positive work; they are not the current state.

## trpc (TypeScript compiler-API real-project measurement)

`trpc` is the largest real-project target (`.local-projects/trpc`, a pnpm
monorepo whose `packages/openapi` and `packages/upgrade` drive the TypeScript
compiler API, plus Next.js/React/Fastify examples). **It is a measured
baseline, never a parity claim** — tsc itself reports over a thousand
diagnostics there, many from examples with unresolved workspace imports, and
the divergence from surge is two-sided.

- Command:
  `pnpm run oracle:compare -- --project .local-projects/trpc --maxDiagnostics 10000`
  (raw counts via `surge -p .local-projects/trpc/tsconfig.json`).

### Measurement of 2026-09-01 (commit `37dfb3a`)

**Point-in-time record; not current.** The current counts are in
[CURRENT_STATUS.md § Real-project compatibility](CURRENT_STATUS.md#real-project-compatibility),
and the surge-only side is broken down in
[§ trpc surge-only inventory](#trpc-surge-only-inventory-2026-09-07) above.

Checkout `dfbafa8ef178a5a3d23ef9461caa9494b3ef7f95` (2026-07-26).

| Metric | Value |
| --- | ---: |
| TypeScript (tsc) diagnostics | 1,244 |
| surge-ts diagnostics | 1,228 |

The raw compare shows divergence in both directions across many codes — this is
a workload, not a compatibility statement, and no share of it should be quoted
as a parity figure. The performance measurement on the same checkout is in
[CURRENT_STATUS.md](CURRENT_STATUS.md#current-performance-state).

Note that the tRPC checkout on disk is **not** the `3e0e979` commit pinned by
the historical benchmark run, so the tsc totals recorded at different dates
below describe different workloads (1,282 on `3e0e979`, 1,244 on `dfbafa8`).

### Historical measurement (2026-08-20)

**Historical.** tsc 1,282 / surge-ts 825 on the `3e0e979` checkout.

Adjudicated against tsc by `(file, line)`, the 2026-08-20 pass moved 6 false
positives out and 4 in, and traded 2 matched diagnostics for 1. Those numbers
are a *delta between two measurements of that pass*, not a description of the
overall divergence. Both residual classes named below are understood:

**Four false positives: members of a re-opened namespace interface.** All four
(`Identifier.text` once, `getText` three times) read a member that only exists in
the *second* `interface` block of a `declare namespace ts` declaration — directly
for `Identifier`, and through inheritance from `Node` for `getText` — which
declaration merging would fold in. The merge is implemented and correct — it removes 33 false
positives with none added — but was gated off at the time because the merged
interfaces pull in a permanently-degraded, and therefore uncacheable,
`typescript.d.ts` expansion.

**Superseded:** the merge is now **on by default** (commit `4c6f584`,
2026-08-28), together with the dotted-name retry; the kill switch is the
opt-*out* `SURGE_NS_IFACE_MERGE=0` (and `SURGE_NS_QUALIFIED_RETRY=0`). The
older "enable with `SURGE_NS_IFACE_MERGE=1`" instruction is historical. See
[docs/perf/NAMESPACE-INTERFACE-MERGE.md](docs/perf/NAMESPACE-INTERFACE-MERGE.md)
for the counter evidence and the cycle-tolerant-resolution work it waited on.

**Two false negatives: unresolved-module policy (still current policy).** Both are a `TS7006` on a
`setMessages((current) => …)` callback in an example whose `~/utils/trpc` import
does not resolve. tsc binds an unresolved module to `any`, and a callback
parameter contextually typed by `any` *is* implicit-any; surge binds it to the
degradation sentinel, which suppresses the report by design so a modelling gap
never cascades. Matching tsc here means binding unresolved imports to `Any`, a
policy change far larger than these two diagnostics — deliberately not taken.

## Historical version notes

The version-tagged milestone notes (`v0.84` audit, the `v0.86`–`v0.92`
auth-kit optimization log, the per-feature `v0.7x`/`v0.8x` notes) and the
superseded "current baseline" support lists have moved to
[docs/history/REAL_PROJECT_COMPAT-HISTORY.md](docs/history/REAL_PROJECT_COMPAT-HISTORY.md).
They are kept verbatim as measured-at-the-time records and **do not describe
current behavior** — several of their "unsupported" entries have since landed.
The current support surface is in [CURRENT_STATUS.md](CURRENT_STATUS.md) and
[PUBLIC_API.md](PUBLIC_API.md).

The Node tooling is dev-only. Rust crates do not depend on Node tooling, and
`cargo test` does not require `pnpm install`.

## Local workflow

- Do not commit third-party project source.
- Put disposable real-project experiments under `.local-projects/`.
- Keep local copies out of committed tests and fixtures.
- Keep the root TypeScript version pinned intentionally; changing it may shift
  oracle output and should be done on purpose, not by accident.

Example:

```bash
mkdir -p .local-projects
cargo run -p surge-ts-cli -- --project .local-projects/<project>/tsconfig.json --compatReport --maxDiagnostics 200
pnpm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
pnpm run oracle:compare -- --file examples/basic.ts
pnpm run oracle:compare -- --file examples/basic.ts --ignoreConfig
```

## What the report tells you

The compatibility report is raw measurement. It helps count the observed
surface without making semantic diagnosis claims:

1. Parser errors
2. Unsupported module syntax
3. Non-relative package imports and side-effect import diagnostics
4. Missing global/lib symbols or unsupported generic syntax
5. Plain type mismatches

The report does not guarantee that a project is expected to pass.
The oracle comparison is also raw measurement. It does not guarantee that
message text or exact spans match; it starts with code, file, and line/column
normalization first.
Diagnostic codes and messages are catalog-driven in `surge-ts-diagnostics`,
so catalog updates can legitimately move oracle output even when checker
semantics stay the same.
Use `--project` for `tsconfig.json`-based projects and `--file` for single
source files. Passing a `.ts` file to `--project` is rejected now so TypeScript
does not misread the file as a config input.
