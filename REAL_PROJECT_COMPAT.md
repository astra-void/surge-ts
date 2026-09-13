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
- **Do not read wall-clock figures out of the version-tagged notes.** They
  predate physical-lib-by-default. Current benchmark figures live in
  [CURRENT_STATUS.md](CURRENT_STATUS.md#current-performance-state) and
  [BENCHMARKS.md](BENCHMARKS.md); everything older is in
  [docs/history/](docs/history/) and [docs/perf/](docs/perf/) as a
  point-in-time record.

## trpc surge-only inventory (2026-09-07)

Measured at commit `f841633` against the pinned TypeScript 7.0.2 oracle, tRPC
checkout `dfbafa8`. `tsc` reports 1,244 diagnostics there and surge-ts 1,133;
this section lists only the **surge-only** side — locations where surge reports
something `tsc` does not. It is a burn-down list, not a parity claim: the
`tsc`-only side (123 at this commit) is tracked separately, and neither side is
gated.

The surge-only side was 65 at `019fb8b` and is 12 here. What closed, and the
oracle preset that pins each, is in the commit range `019fb8b..f841633`:
`this-type-predicate-narrowing-basic`, `guard-polarity-narrowing-basic`,
`overload-merge-contextual-callback-basic`, `namespace-callback-parameter-basic`,
`exit-and-alias-narrowing-basic`, `generator-missing-return-basic`,
`assertion-and-nonnullable-basic`, `class-prototype-instanceof-basic`,
`tuple-union-destructure-basic`, `intersection-two-union-operands-basic`,
`void-parameter-arity-basic`, `instantiation-expression-basic`,
`literal-equality-narrowing-basic`, `optional-chain-guard-narrowing-basic`,
`promise-like-intersection-basic`, `namespace-merged-function-export-basic`,
`json-module-import-basic`, `json-module-resolution-disabled-basic`,
`instanceof-heritage-narrowing-basic`, plus the local-shadow case added to
`umd-global-module-reference-basic`. Two of
the fixes are not preset-pinned because their trigger is a shape surge fails to
model and `tsc` types fine — a fixture would pin the modelling gap rather than
the suppression; both are called out in their commits.

The 12 that remain, with the root cause where it is known:

| Location | Code | Root cause |
| --- | --- | --- |
| `examples/next-sse-chat/src/server/db/schema.ts:13`, `:61` ×2 | TS4111 | drizzle's `pgTable(...)` result degrades to a string index signature, so column access reads as index-signature access under `noPropertyAccessFromIndexSignature`. Only reproduces inside the full project — the same file checked on its own is clean. |
| `packages/server/src/__tests__/trpcServerResource.ts:53`, `:64` | TS7006, TS2349 | `createHTTPHandler`'s options are an intersection whose `CreateContextCallback<…>` operand is a conditional over an unresolved router context. Replacing that operand with a plain object closes `:53`, so the contextual type is lost through the operand — but the object-literal path that loses it has not been isolated. |
| `packages/server/src/unstable-core-do-not-import/router.ts:75` | TS2314 | The file declares its own one-parameter `DecorateRouterRecord` and uses it at two places; the use *inside* that alias resolves to it and the use in the sibling `RouterCaller` alias resolves to `packages/react-query`'s two-parameter declaration instead. Renaming either declaration clears it, so it is a cross-module type-name leak, not an arity bug. The two files do not import each other. |
| `packages/upgrade/src/bin/index.ts:67` | TS2339 | Module top-level statements carry no flow analysis — the parser drops `if`/`while`/`try` at module scope entirely — so the guard before this line narrows nothing. `.sort` is then looked up on the un-narrowed union. Reproduces at module scope only; the same code inside a function types correctly. |
| `packages/server/src/observable/observable.test.ts:1` | TS2305 | `import { EventEmitter } from 'stream'`. `@types/node` writes `class Stream extends EventEmitter` with `export = Stream`, and `EventEmitter` reaches the import as a *static* member inherited from the base class, contributed there by its own namespace merge. surge models neither namespace-merged class statics nor their inheritance. |
| `examples/nuxt/nuxt.config.ts:2` | TS2304 | `defineNuxtConfig` comes from Nuxt's generated `.nuxt` types. |
| the remaining 3 | TS2322 ×2, TS2345 | one-off assignability divergences inside tRPC's generic builder machinery (`next/ssrPrepass`, `next/withTRPC`, `server/resolveResponse`); not yet reduced |

## trpc surge-only inventory (2026-09-10)

Measured on 2026-09-10 against the pinned TypeScript 7.0.2 oracle, tRPC
checkout `dfbafa8`, from a **dirty working tree** on top of `6e034fd` (a copy
of the tree's binary taken at 13:39, so the number is not a clean-worktree
figure and is not carried into the status table). `tsc` reports 1,244
diagnostics and surge-ts 1,131. The file/code/line drift is 119: 116 that
`tsc` reports and surge does not, unchanged through this pass, and **3**
surge-only. On the 1,128 locations both compilers agree on, message text
matches 1128 / 1128. zod (21/21), ky (0/0), ofetch (1/1), unnamed (0/0) and the
tanstack-query aggregate (112) are unchanged by the changes below.

The surge-only side was 12 at `f841633`. The dirty tree had since closed the
drizzle TS4111 ×3, the nuxt TS2304 and the `ssrPrepass` TS2322 on its own, and
had also *opened* ten false TS7006 (eight in `procedureBuilder.ts`, one each in
`observable.ts` and `trpcServerResource.ts`): the new declaration-flow probe
re-inferred every annotated initializer without its annotation as the
contextual type, so a method-shorthand parameter inside `const b: Builder = {
input(input) { … } }` was reported as an implicit `any` by the probe itself.
The probe now runs only for a declared union and discards what it reports.

What closed in this pass, each pinned by an oracle preset:

| Location | Code | Root cause and fix | Preset |
| --- | --- | --- | --- |
| `procedureBuilder.ts` ×8, `observable.ts:25`, `trpcServerResource.ts:85` | TS7006 | The declaration-flow probe above. | `annotated-object-method-parameter-basic` |
| `packages/next/src/withTRPC.tsx:158` | TS2322 | `if (opts.ssr)` over `WithTRPCSSROptions \| WithTRPCNoSSROptions` narrowed nothing: a property truthiness test only decided a single-unit leaf, so `ssr: true \| fn` and `ssr?: false` both stayed undecided. Leaves now decide by tsc's type facts (callables, arrays and non-empty shapes are truthy; a union decides when all members agree; below an optional property only a falsy leaf decides). | `property-truthiness-discriminant-basic` |
| `resolveResponse.ts:115` | TS2322 | `eagerGeneration ? [] : …` where `const eagerGeneration = !untransformedJSON`: the alias condition was expanded only for `if` statements. The function-body declaration path now records the alias on the symbol table and expression-level narrowing expands an identifier through it. | `alias-condition-conditional-expression-basic` |
| `router.ts:75` | TS2314 | The cross-module type-name leak reduces to an imported interface whose member returns a function-type alias whose body names a same-file generic; resolving it consulted the consumer's file-local declarations first, so `DecorateRouterRecord<TRecord>` bound to `react-query`'s two-parameter declaration. The local table is now skipped whenever resolution has crossed into another file's declaration scope, not only into a dependency `.d.ts`. | `cross-module-alias-body-scope-basic` |

The 3 that remain, with the root cause where it is known:

| Location | Code | Root cause |
| --- | --- | --- |
| `packages/server/src/__tests__/trpcServerResource.ts:64` | TS2349 | The call is `onRequestSpy(...args)` on a vitest `Mock<typeof handler>` (not the `handler` call on the next line). `Mock<T>` is `MockInstance<T> & (T extends Constructable ? … : …)` and `T` is `http.RequestListener`, whose second parameter is `InstanceType<Response> & { req: InstanceType<Request> }` with `Response` defaulted to `typeof ServerResponse`. Two of the links in that chain are closed below (`typeof` of a value-less class, and an argument-less generic class taking the lazy path), and each of `(res: ServerResponse) => void`, `InstanceType<typeof ServerResponse>` and `Mock<(req: IncomingMessage, res: ServerResponse) => void>` now types correctly. What remains is the last link: inside an alias's *default-bound* substitution the analysis-phase context carries no `file_kinds`, so `ServerResponse` is not seen as library-scoped and expands eagerly; that expansion embeds the sentinel (its `stream.Writable` heritage does not resolve inside the ambient `http` module, `SURGE_AMBIENT_BLOCK_IMPORTS=1` included), the intersection merge carries it into the function parameter, and the declaration checker's `type_contains_unknown` gate degrades every value declared through such a function type. Reduced to `type P<S extends typeof ServerResponse = typeof ServerResponse> = (res: InstanceType<S> & { x: number }) => void` — the same alias with `{ a: InstanceType<S> } & …`, or with `S` written explicitly, is fine. Two more links were closed afterwards — the library-scoped gate now also answers by path where `file_kinds` is absent, and `typeof C` renders in a syntactic display so `RequestListener`'s `typeof` defaults let it defer — after which `getSpy<RequestListener>()` on a *named* import against a local copy of `Mock` is callable. What still fails, each on its own: the same alias reached as `http.RequestListener` through a default-import namespace, and vitest's real `Mock<T>`, whose trailing `& { [P in keyof T]: T[P] }` operand maps over the alias. |
| `packages/server/src/observable/observable.test.ts:1` | TS2305 | `import { EventEmitter } from 'stream'`: three layers — a namespace's `export { internal as EventEmitter }` re-export as a class static and type, class+namespace merged statics, and inherited statics from an imported base inside an ambient module. Unchanged from the 2026-09-07 inventory. |
| `packages/upgrade/src/bin/index.ts:67` | TS2339 | Module top-level statements carry no flow analysis (the parser drops `if`/`while`/`try` at module scope). Unchanged. |

### Closed while reducing `trpcServerResource.ts:64` (2026-09-10)

Two links of that chain were false-positive classes in their own right and are
closed, each pinned by a preset:

- **`typeof C` without a value symbol.** A class imported with `import type`,
  or named inside the ambient module that declares it (`RequestListener<Request
  extends typeof IncomingMessage = typeof IncomingMessage>` in `@types/node`),
  has a type declaration but no value in reach; surge degraded the query to the
  sentinel, so `InstanceType<typeof C>` was lost and anything declared through
  such a default was silently assignable to everything. The query now stands a
  constructor surface over the instance type (a construct signature returning
  `C` plus `prototype`, kept open so a static surge cannot see is never
  reported); a tainted expansion is still left degraded.
  `typeof-class-without-value-basic`.
- **An argument-less generic reference never took the lazy path.** The display
  name a lazy reference needs was built from the *written* arguments only, so
  `ServerResponse` (defaulting `Request = IncomingMessage`) expanded eagerly
  where `ServerResponse<IncomingMessage>` deferred, and the eager expansion of
  a library class whose heritage does not resolve collapsed `declare const r:
  ServerResponse` to the sentinel. The display is now rendered from the
  declaration's defaults (`ServerResponse<IncomingMessage>`, as tsc displays
  it) for interfaces and classes and for aliases in a dependency's `.d.ts`; a
  *user* alias keeps its eager path, because deferring it too changed one zod
  call-site union (`$ZodSuperRefineIssue` against an `Identity<…>` parameter,
  a false TS2345). `generic-default-arguments-display-basic`.

One more class was noticed and is **not** a trpc item: a default import of an
`export =` module under `moduleResolution: bundler` without `esModuleInterop`
reports TS2305 (`tsc` allows it through the resolver's
`allowSyntheticDefaultImports` default).

### trpc surge-only reaches 0 (2026-09-11)

The last three items closed in this order, each pinned by a preset; measured on
the dirty tree against the same `dfbafa8` checkout, `tsc` 1244 / surge 1128
with all 1128 shared locations matching on message text, and the `tsc`-only
side unchanged at 116.

- **`trpcServerResource.ts:64`** — the final link was `http.RequestListener`
  read through a *default* import. `import http from "http"` under
  `esModuleInterop` binds the module object as the synthetic default, and its
  exported types are reachable as `http.X` exactly as through `import * as`;
  surge bound the value but registered no type members, and an unresolved
  qualified type name is deliberately silent, so every `http.X` annotation
  was the sentinel. The synthetic default now contributes the module's types
  as a `local.<member>` alias layer. `synthetic-default-import-type-members-basic`.
  Two more links that the reduction crossed on the way: a class's type
  parameter *default* is now bound under the class's namespace prefix (express's
  `Request<P = ParamsDictionary, …>` names its siblings bare), and a default
  that is itself an instantiation (`Record<string, any>`) renders into the
  lazy display. Closing the qualified route exposed `express.ErrorRequestHandler`
  in the express adapter test, whose cause is below.
- **`express.test.tsx:97` (TS7006 ×4, exposed by the previous fix)** —
  `@types/express` declares its handlers as empty interfaces extending a
  *function-type alias*; surge inherited call signatures only from an object
  base, so an arrow written against the interface had no contextual parameter
  types. A function-typed base now contributes its signature.
  `interface-extends-function-alias-basic`. Still open and **not** a trpc item:
  an arrow against express's generic `RequestHandler` (an interface extending
  `core.RequestHandler<P, …>` with five defaulted parameters) is a false
  TS2322 whose inferred type shows the base alias's own parameter names.
- **`observable.test.ts:1`** — `import { EventEmitter } from "stream"` reads a
  static that `class Stream extends EventEmitter` inherits from a base bound by
  an import inside the ambient block, through `export *` (`node:events` →
  `events`), and which that base carries as a namespace-merged member
  (`namespace EventEmitter { export { internal as EventEmitter } }`). Classes in
  a block were bound before its imports were resolvable and before the
  namespace merge was applied. Statics are now re-merged after the merge and
  again once every ambient table is resolved; a base surge can only model as
  `any` (that namespace re-export) leaves the derived static side open, so the
  import reads `any` instead of a missing export. `export { Class }` also
  exported the pre-merge class before. `ambient-class-static-inheritance-basic`.
- **`upgrade/src/bin/index.ts:67`** — the parser dropped a module-scope `if`
  entirely. It is kept now with the function-body branch lowering, and when
  exactly one branch diverges (`throw`, a `never`-returning call such as
  `process.exit`) the other branch's narrowing applies to the module's symbols.
  The condition is evaluated for narrowing only — its diagnostics are
  discarded, because reporting it exposed four TS4111 on `args.verbose` off a
  parsed-options object surge models as an index signature — and the branch
  bodies are still not checked at module scope, as before.
  `module-scope-if-divergence-narrowing-basic`.

What this does and does not say: trpc's surge-only side is 0 at this commit
and the `tsc`-only side is 116, so trpc is now a false-positive gate in the
ky/unnamed family with a known, inventoried false-negative side; it is still
**not** a parity claim.

## trpc `tsc`-only inventory (2026-09-11)

Measured on 2026-09-11 against the pinned TypeScript 7.0.2 oracle, tRPC
checkout `dfbafa8`, from a dirty working tree on top of `6e034fd` (so it is not
a clean-worktree figure and is not carried into the status table). `tsc`
reports 1,244 diagnostics and surge-ts 1,128; the surge-only side is **0** and
every one of the 1,128 shared locations matches on message text. This section
is the other side: the **116** diagnostics `tsc` reports and surge does not,
across 106 distinct file/code/line locations, grouped by the root cause each
was reduced to.

| Count | Root cause | Codes |
| --- | --- | --- |
| 62 | tRPC's own router/client proxy types | TS2339, TS7006, TS2532, TS2686, TS4111, TS7031, TS2554, TS2578, TS2769 |
| 16 | jscodeshift's `VariableDeclarator \| IdentifierKind` AST unions (`upgrade/src/transforms/provider.ts`) | TS18048, TS2339 |
| 11 | declaration-emit portability | TS2883, TS4023 |
| 10 | drizzle's `findMany({ orderBy: (fields, ops) => … })` callbacks | TS7006 |
| 7 | Prisma's generic class value side (`new PrismaClient().task`) | TS2339 |
| 3 | react-hook-form generics | TS2345, TS2339 |
| 2 | Prisma input types | TS2353, TS2322 |
| 2 | `Object.fromEntries` index signature | TS4111 |
| 2 | `Uint8Array` / `BufferSource` assignability | TS2345 |
| 1 | `unknown` source rejection (`catch` variable into a typed parameter) | TS2345 |

### The dominant cluster is one gap, and it is not a checker bug

The 62-diagnostic group is not 62 problems. Every one of them is a read off a
value surge cannot model: `createTRPCNext<AppRouter>()`, `createTRPCClient<AppRouter>()`,
and everything derived from them. Probed directly in the corpus, `proxy.anything`
and `postList.anything` are both silent and `const s: string = postList` is
accepted — the whole proxy is open. Once that value is open, every diagnostic
downstream of it disappears: the `TS2532` on `postList[0].title`, the `TS7006`
on a `.map((page) => …)` callback parameter, and the `TS2686` on the React UMD
global inside the JSX that callback returns, which surge never walks.

`AppRouter` is `typeof appRouter`, and `appRouter` is built by tRPC's own
generic builder chain (`initTRPC…router({ … })`). Closing this cluster means
modelling that chain — `TRouter['_def']['record']` and the `DecorateRouterRecord`
mapped type over it — not fixing 62 separate sites.

### Two findings from reducing it, both reproduced standalone

**The published type of an exported value is degraded whenever its type names
an imported type.** Module analysis publishes export types map-less
(`module_scope_by_file` is taken for value collection and export-table
construction), and a declaration carries the *preliminary* resolution scope
attached at collection time, which holds the file's own declarations and no
import layers. Resolving an imported alias body under it misses every imported
name and lands on `unknown`, which the check phase then trusts for consumers.
The reduction is five files (a conditional alias, a generic function that
returns it, a `typeof` alias, a producer, a consumer) and is what silences the
12 `TS2339`s tsc reports on `trpc.withTRPC` / `trpc.<router>`.

Repairing the scope is implemented and available behind
`SURGE_UPGRADE_ANALYSIS_SCOPES=1`, and is **off by default**: honest exports
close those 12 and open 13 others. With the export honest, a router surge
cannot model decides `ProtectedIntersection`'s `keyof A & keyof B extends never`
from a key set it never had, and the string-literal error type that conditional
produces then swallows every property read off the router in the examples where
tsc *can* model it. The separating signal exists at the type-argument boundary
(`AppRouter` resolves tainted where surge degraded it, clean where the module
is genuinely unresolved) but it does not survive substitution: a binding is a
bare `Type::Unknown`, and both per-name and whole-substitution provenance were
measured and neither reached the conditional. Turning the flag on for good
needs that provenance, or the router modelling above.

### What landed against this inventory (2026-09-11, second pass)

Five root causes were reduced to standalone reproductions and fixed. None of
them moves the trpc `tsc`-only count yet — the jscodeshift chain needs every
link before its 16 report — but each is a correctness gap in its own right, and
together they took the tanstack-query aggregate from 79 false positives to
**25**.

| Fix | Preset |
| --- | --- |
| `import x = require("m")` on a module with no `export =` binds the module namespace, as `import * as x` does. A module that *writes* `export = target` with an unresolvable target keeps the unknown placeholder, so the no-cascade behaviour both `export_equals_*_no_cascade` tests pin is unchanged. | `import-equals-module-namespace-basic` |
| A module namespace one module re-exports (`import * as inner …; export { inner }`) is rebuilt against the final export tables. The object is materialized at import-binding time, and the bindings the final analysis round runs under come from the preliminary analyses, where the thin value pass degrades every variable to `unknown`; a re-exporting module baked that in and nothing refreshed it. | `reexported-namespace-object-members-basic` |
| A `declare namespace`'s *value* members carry their written annotation into the namespace object instead of a permissive `any`. Sibling names resolve under the namespace prefix, as a member signature already did. | `namespace-value-member-annotation-basic` |
| Any `number` is assignable to a numeric `enum`, the enum type and a member type alike — its own `A \| B` is typed `number`. A string enum still rejects `string`. Exposed by the line above, which made `ts.TypeFlags` real in tRPC's `openapi` package. | `numeric-enum-number-assignability-basic` |
| Inherited statics are looked up in the module's **own** table. A generic class models its value side as `any` and contributes no value symbol, so a parent-traversing lookup found the *ambient global* of the same name and merged the base's statics into it — publishing the DOM's `MutationObserver` as a module's export, 51 false `TS2554` in the tanstack aggregate. | `generic-class-static-inheritance-no-global-merge-basic` |

One more link was reduced and left open rather than guessed at: a namespace
re-exported *by name* (`import { ns } from "./deep"; export { ns }`) loses its
qualified `ns.Member` type keys for the consumer, because the export side reads
the file's own declarations while an imported namespace's member keys live in
the import layer. `ast-types` re-exports its whole `namedTypes` surface that
way. Copying the keys from the local table alone is a no-op — measured — and
reaching the import layer needs an iteration over scope layers that costs one
full scan per exported specifier, so it wants a shape that does the scan once
per export table.

The jscodeshift surface is now most of the way open: `api.jscodeshift`, `j`,
`root`, and `root.find(…)` all resolve where every one of them was silent. What
still stands between that and the 16 diagnostics is `j.VariableDeclaration`,
which reads `Type<VariableDeclaration>` off `typeof namedTypes` in a
*dependency* `.d.ts` — the namespace-member annotations resolve in a source file
but not there — and, after it, `Collection<T>`'s inference through `find`,
`noUncheckedIndexedAccess` on the declarator array, and the property check on
the resulting union.

**`Object.fromEntries` needs two things, and one of them landed.** A type
parameter written as the element of an `Iterable<T>` parameter is now inferred
from an array or tuple argument (`iterable-element-inference-basic`); an array
is `Type::Array` and has no object surface, so the interface-member walk that
handles every other generic reference inferred nothing. That is not yet enough
for the two `TS4111`s: `Object.fromEntries` is an *overload group* on an
interface, and surge resolves a call to an overloaded interface member to a
permissive `any` — `interface P { m(x: string): string; m(x: number): number }`
called as `p.m("a")` yields no type at all. That is the known blocked overload
program, not a new finding.

## tanstack-query corpus (provisioned 2026-09-09)

TanStack/query at `cdbe8cb`, installed with
`pnpm run real:tanstack-query:provision` (shallow clone, `packages/**` only,
`--ignore-scripts`; the repo's own `packageManager` pin is honoured through
corepack). The upstream root `tsconfig.json` covers only `*.config.*`, so the
harness target is an aggregate, `tsconfig.surge.json`, kept in the repo at
`scripts/real-projects/targets/tanstack-query.tsconfig.json` and copied into the
corpus by the provisioning script.

The aggregate spans ten packages — `query-core`, `query-persist-client-core`,
the two storage persisters, `query-broadcast-client-experimental`,
`query-test-utils`, `react-query`, `react-query-persist-client`,
`react-query-next-experimental`, `eslint-plugin-query`. Two exclusions are
deliberate and both were reduced from failures, not guessed:

- `react-query-devtools` pulls `@tanstack/query-devtools` source into the
  program, which is Solid. Under the aggregate's `jsx: react-jsx` its JSX
  namespace collides with React's and the oracle itself reports 408 diagnostics.
  The package is still measurable on its own tsconfig.
- `packages/eslint-plugin-query/src/__tests__/ts-fixture` contains a
  `declare module '@tanstack/react-query'` fixture. In a single program that
  ambient declaration shadows the real workspace package for every consumer, so
  the oracle reports 38 phantom TS2305/TS2724. Per-package it is harmless.

With those two exclusions the oracle reports **0** diagnostics on the aggregate,
making this a false-positive corpus in the ky/unnamed family rather than a
parity target.

**surge could not finish the aggregate until 2026-09-10.** It overflowed the
main thread's stack, and a depth bound on the merge only converted that into an
exponential fan-out that peaked at 55 GB RSS. The shape reduces to three lines
against the corpus's own vitest types:

```ts
import { vi } from 'vitest'
const windowSpy = vi.spyOn(globalThis, 'window', 'get')
windowSpy.mockImplementation(() => undefined as unknown as Window & typeof globalThis)
```

The cause is `Window & typeof globalThis` naming itself through its own
`window` and `self` members while surge merges an intersection eagerly into one
object surface: a property both operands declare is merged as its own
intersection, so the merge re-entered itself with the operands it was already
merging, alternating `window`/`self` with a period of two, and every level
re-merged the ~1000 members of the global object. The plain annotation never
reached that merge (its `typeof globalThis` is still unresolved while the DOM
globals are collected); the generic route — `Parameters<T>`/`ReturnType<T>`
substituted into `mockImplementation`'s parameter type — did. Four changes
close it, each pinned by the `intersection-self-reference-cycle-basic` preset:
the merge detects re-entry by operand identity, nested deferred intersections
are flattened (`A & (B & C)` is `A & B & C`) and identical operands
deduplicated so every level is the same nominal reference, one deferred merge is
shared per operand set instead of re-merged per reference instance, and
`typeof globalThis` is a nominal reference rather than a bare structural object
(it also now displays as `typeof globalThis`, as tsc does).

With those the aggregate completes in about 5 s at about 1.1 GB peak RSS. It
reports **120** diagnostics against the oracle's 0 — 133 when it first became
measurable, less the seven and the six closed below. This was measured on a
**dirty working tree** on top of `6e034fd` and is a burn-down list, not a gate;
re-measure from a clean worktree before pinning it:

| Code | Count | Code | Count |
| --- | ---: | --- | ---: |
| TS2345 | 39 | TS7030 | 2 |
| TS2322 | 32 | TS2741 | 2 |
| TS2349 | 16 | TS7006, TS2493, TS2356, TS2353 | 1 each |
| TS2339 | 11 | | |
| TS18048 | 9 | | |
| TS2304 | 5 | | |

By package: `query-core` 76, `react-query` 14, `eslint-plugin-query` 9,
`query-sync-storage-persister` 7, `query-persist-client-core` 6, the two
experimental/async packages 3 each, `react-query-persist-client` 2.

Known and not yet fixed (verified against tsc 7.0.2 on 2026-09-10): the DOM
globals `window` and `self` are resolved while `globalThis` has no value
symbol yet, so surge types them, and the `window`/`self` members read through
`Window & typeof globalThis`, as `Window` where tsc says
`Window & typeof globalThis`. Assigning such a read to a
`Window & typeof globalThis` annotation is a false TS2322.

### Closed: seven false TS2307 on `react-error-boundary` (2026-09-10)

A package `exports` entry may resolve, under the `types` condition, to a
*runtime* JavaScript file; TypeScript then strips the runtime extension and
probes the declaration beside it. `react-error-boundary` spells its `"."` entry
as a nested condition object whose `import` target is
`./dist/react-error-boundary.cjs.mjs`, and ships
`react-error-boundary.cjs.d.mts` next to it.

surge substituted the extension with `Path::with_extension`, which rewrites the
segment *before* the runtime one: the stem of `…cjs.mjs` is `…cjs`, and asking
that for `d.mts` gave `react-error-boundary.d.mts`, a file no package ships.
Every import of the package was a false TS2307. The same slip hit the far more
common `.esm.js` / `.cjs.js` bundle naming, where it probed `name.d.ts` instead
of `name.esm.d.ts`. The fix strips only the runtime extension and appends to
what is left, and is pinned by the `exports-dotted-runtime-target-basic`
preset. Aggregate 133 to 126, with the seven TS2307 the only change; ky,
ofetch, zod, unnamed and trpc diagnostics stayed byte-identical.

### Closed: six false TS2345 on `Array<T>` parameters (2026-09-10)

A parameter written as `Array<T>` or `ReadonlyArray<T>` inferred nothing from an
array argument. Only the `T[]` shorthand had an element-wise inference arm; a
reference parameter fell through to the generic-member walk, which matches an
argument's object surface and an array argument has none.

`addToStart(items, item)` against
`addToStart<T>(items: Array<T>, item: T, max?: number)` therefore saw only the
second argument. With `const item = 4` that made `T` the literal `4`, and the
first argument was reported as `number[]` not assignable to `4[]`. Once the
array parameter contributes `number` as well, the common-primitive rule already
in `record_type_argument_candidate` settles `T` on `number`. Pinned by the
`array-reference-parameter-inference-basic` preset. Aggregate 126 to 120, six
TS2345 the only change, corpora byte-identical.

### Closed: eight false TS18048 through `await` of a nullable union alias (2026-09-10)

`await` is erased at parse time and `Promise<T>` is modelled as its awaited
`T`, so the awaited form is produced only where the promise instantiation is
*resolved*. A library generic interface instantiation is normally deferred to a
lazy reference, and a deferred `Promise<T>` stays an opaque union member until
something peels it. TanStack Query's `type Promisable<T> = T | Promise<T>`
instantiated as `Promisable<PersistedClient | undefined>` therefore awaited to
`PersistedClient | undefined | Promise<T>`: `restored?.clientState.queries`
reported `'restored.clientState' is possibly 'undefined'` (eight sites across
the persister tests), and `if (restored)` still typed `restored.timestamp` as
`number | undefined`. Reduced in `awaited-nullable-union-alias-basic`.

The fix collapses such a promise eagerly, scoped to a promise resolved inside a
*source* type-alias body whose awaited type includes `undefined`. Both halves
of the scope are measured, not chosen:

- collapsing every `Promise`/`PromiseLike` erased the `void | Promise<void>`
  and `return this.promise` distinctions the deferred form keeps under the
  implicit-await model — ky 0→1 (`resolve()` arity), zod 21→22, trpc +1
  (TS7030);
- collapsing one inside a dependency's own alias (`MaybePromise`, `Thenable`)
  was neutral on the tanstack count and only added risk;
- `Array`/`ReadonlyArray`, which the same eager path collapses to `T[]`, were
  tried alongside and moved zod (+3), ky (+1) and trpc (+1).

Aggregate 112→104 with the eight TS18048 the only change; ky, ofetch, trpc and
unnamed byte-identical against a gate-off build of the same tree. A zod
diagnostic at `v4/core/api.ts:1666` appeared during this work and was chased
for three build cycles before a gate-off build of the same tree showed it is
present without the change — it belongs to concurrent in-flight work, not to
this one.

### Closed: literal-equality narrowing of a property reference (2026-09-10)

`if (filters?.refetchType === 'none') return` followed by
`filters?.refetchType ?? filters?.type ?? 'active'` reported the `??` chain
as `'all' | 'active' | 'inactive' | 'none'` against `QueryTypeFilter`
(`queryClient.ts:475`). surge narrowed a bare identifier by literal equality
and filtered a base union by a discriminant property, but never narrowed the
property reference itself. It now joins the reference guards
(`ReferenceGuard::LiteralEquality`), composing with the discriminant filter on
the same condition; the complement keeps `undefined` and returns it to an
optional slot's flag.

Landing it surfaced a write-side gap: `this.p = …` was checked against the
narrowed `this`, so zod's `if (this.value === "valid") this.value = "dirty"`
became two false TS2322 (`parseUtil.ts:95`, `:98`). `this`-property writes now
check against the declared type, as `o.p = …` already did. Both are pinned by
`property-literal-equality-narrowing-basic`. Aggregate 104→103; zod, ky,
ofetch, trpc and unnamed byte-identical against the same-tree baseline.

Seen and not fixed while reducing it: `if (opts?.nested.mode === 'x')
opts.nested.mode` still reports `'opts' is possibly 'undefined'` — the
optional-chain base is only proven present for a one-segment discriminant path.

### Closed: `vi.fn()` — a generic call through a `typeof fn` property (2026-09-11)

The cluster that dominated the corpus (63 of the diagnostics named
`MockInstance<T>` with a bare `T`) was a call-checker gap, not a vitest
modelling one. `vi` is `declare const vi: VitestUtils` and the member is
`fn: typeof fn`. That annotation resolved eagerly to the declaration's bare
function type — dropping its parsed signature — and the property-call path
handed it to `check_function_type_call`, which takes no signature and performs
no generic instantiation. `T` was never bound, `Mock<T>`'s conditional operand
(the one carrying the call signature) could not resolve, and what survived was
`MockInstance<T>`: neither callable nor assignable to any callback.

Three pieces close it, pinned by `typeof-function-member-generic-call-basic`:

- **The `typeof` arm keeps the declaration on the function handle.** An
  opaque slot on `FunctionType`, outside the payload, identity and equality —
  handle metadata like the written parameter names. Not for an overload group:
  a group's value type is the permissive fold of every overload and only the
  first declaration's parsed signature survives as template, so instantiating
  `vi.spyOn(obj, "method")` against the `"get"` accessor overload bound the
  wrong parameters — 146 false TS2345 on the first attempt. The group's
  template now carries an `overloaded` flag and the attach skips it.
- **The property-call path instantiates through that signature**, exactly as
  a call on the declared symbol does.
- **A type parameter with no inference source falls back to its default**,
  only when that completes the binding. This is the third form of the default
  fallback rejected twice above, and the two rejections are what shaped it:
  a parameter that *had* a source (an argument mentioning it, a contextual
  return type) but that surge failed to infer must not be defaulted — zod's
  `hash(alg, { enc: "base64" })` against `Enc = "hex"` — and defaulting some
  parameters while another stays unbound moved ofetch's `$fetch(url, options)`
  off the all-unknown bail into a wrong conditional branch.

Aggregate **103 → 29**: 74 locations gone, none new, against a gate-off build
of the same tree. zod, ky, ofetch and unnamed byte-identical; trpc 1134 → 1130,
the four being `express.test.tsx:97` implicit-`any` parameters that were not
present before the day's concurrent edits either.

The four `MockInstance<T>` messages that survived were the arrow-argument
shape, closed next.

### Closed: `T` from an arrow argument (2026-09-11)

`vi.fn((value: Date) => value.toISOString())` bound `T` to nothing on both the
symbol and the member path. Generic-call inference types each argument with
the expression *sketch* (`infer_expression`), not the authoritative checker,
because at that point there is no contextual signature to check against. The
sketch's arrow type ignored the written parameter annotations — every
parameter was `any` — and inferred an expression body without its parameters
in scope, so `value.toISOString()` was an unresolved read, the return degraded
to unknown, and an argument whose type contains unknown is skipped as an
inference source.

The sketch now reads the annotations, binds the parameters into the body
scope for the expression and the block form alike, and maps a written return
annotation. Three constraints keep it a sketch:

- an annotation that fails to resolve stays `any`, which is what every
  parameter was before, so nothing that used to infer stops inferring;
- the diagnostics the annotation mapping emits are discarded — the
  authoritative pass reports the genuine ones (mapping `T` in a *generic*
  arrow's annotation raised TS2304 in `notifyManager.ts` on the first attempt,
  so a generic arrow keeps the untyped sketch);
- the sketch's own limits stay: a body that calls a global (`(n: number) =>
  String(n)`) still degrades, because its call arm resolves callees from the
  local table only and `String` is an object with a call signature, and a
  member generic call inside the body (`() => sleep(10).then(() => 'data')`)
  does not instantiate the member's type parameters.

Pinned by `arrow-argument-parameter-inference-basic`. Measured as an A/B of
one binary with the previous sketch behind a temporary switch (same tree,
same moment), because the tree changed under the earlier baseline:

| corpus | previous sketch | new sketch |
| --- | --- | --- |
| tanstack-query aggregate | 80 | 77 (3 gone, 0 new) |
| trpc | 1128 | 1128, byte-identical |
| zod / ky / ofetch / unnamed | 21 / 0 / 1 / 0 | byte-identical |

The aggregate's 80 is not the 29 recorded above plus new drift from this
change: 51 of them are TS2554 `Expected 1 arguments, but got 2` on every
`new MutationObserver(client, options)`, introduced by a concurrent,
uncommitted edit to `inherit_base_statics` (`modules/exports/values.rs`) that
looks the derived class up through the layered table and, for a generic class
whose own value is absent, merges the DOM `MutationObserver` static side into
the module's export under the class's name. It reproduces with the sketch
change switched off and is not measured here as part of it.

Still open in the same area: the two sketch limits above; the last
`MockInstance<T>` in the corpus (`mutation.test.tsx:1250`,
`vi.fn(() => sleep(10).then(() => 'data'))`) is the member-generic one.

### Closed: a nested predicate naming the enclosing generic's parameter (2026-09-11)

`function isTargetFunction(node: string): node is TFunc` declared inside
`createOrderRule<TFunc, TProp>` and used as an `if` guard reported TS2304 on
`TFunc` — at the declaration's own span, after that declaration had already
checked clean.

Narrowing re-resolves a predicate's written target at the guard site, to decide
whether the guard selects among the subject's union members. That resolution
was handed only the guard's own type-argument substitution, which holds the
*predicate's* type parameters; this predicate has none of its own, and the
enclosing generic's parameters live on the checker's type-parameter scope
stack. The narrowing path was the one annotation-resolution site that never
consulted that stack, so the enclosing parameter read as an unresolved name.

It now merges the active scopes the way `try_map_parsed_type_with_substitution`
already did. The parameter resolves to a placeholder rather than a concrete
type, so the guard still declines to narrow: this removes a false positive, it
does not add narrowing. Pinned by
`nested-predicate-enclosing-type-parameter-basic`.

Measured with a temporary switch inside one binary (same tree, same moment —
the tree had moved under the previous baseline):

| corpus | fix off | fix on |
| --- | --- | --- |
| tanstack-query aggregate | 77 | 76 (1 gone, 0 new) |
| trpc | 1128 | 1128, byte-identical |
| zod / ky / ofetch / unnamed | 21 / 0 / 1 / 0 | byte-identical |

### Half-closed: a relative `declare module "./sibling"` augmentation (2026-09-11)

Root-causing the `eslint-plugin-query` cluster found that
`@typescript-eslint/types` hangs `parent` on every AST node with
`declare module './generated' { interface BaseNode { parent: TSESTree.Node } }`
in a sibling `.d.ts` — and surge collected that augmentation but never applied
it.

Augmentations are filed under the specifier written in the `declare module`
header and looked up under the specifier a *consumer* writes. For a bare
specifier those are the same string. A relative one names a file relative to
the augmenting file, so no consumer outside that directory writes it, and the
entry sat in the map unused.

A relative specifier is now resolved the way an import of it is resolved and
filed under the target's canonical identity, which every import path already
holds. All four resolution routes consult it — package and relative, in both
`try_resolve_module` and `try_resolve_module_export_table`. Pinned by
`relative-module-augmentation-basic`.

**This closes only the direct half.** Splitting the repro shows it exactly:

| augmentation | before | after |
| --- | --- | --- |
| `interface Identifier { viaDirect }`, read as `id.viaDirect` | missing | found |
| `interface BaseNode { viaBase }`, read through `Identifier extends BaseNode` | missing | still missing |

The merge lands in the per-import export-table clone, while heritage resolves
`BaseNode` in the declaring file's own scope, which has no augmentation merged.
Closing that half means merging the augmentation into the target file's own
declaration table — the model tsc uses, and a much wider blast radius, since it
changes the module's internals for every consumer including itself. It needs
its own measured program.

Corpus movement from the direct half alone: **none**. Every corpus is
byte-identical with the fix switched off and on in one binary. The
`eslint-plugin-query` diagnostics are all the heritage half.

The aggregate reads **25** from a build of the tree carrying both this work and
the concurrent session's fix to `statics_merged_into` (an own-table lookup, so a
generic class whose own value symbol is absent no longer inherits the DOM
`MutationObserver` static side — the 51 false TS2554 recorded above are gone).

### Open: the `@typescript-eslint/utils` namespace re-exports

Nine of the aggregate's remaining diagnostics sit in `eslint-plugin-query` and
every one of them reaches its type through `@typescript-eslint/utils`, whose
entry point is six `export * as NS from './…'` re-exports. They are not yet
root-caused and may not be one root, but they share that path:

- `Property 'parent' does not exist on type 'Identifier'` (×2) — a heritage
  member of `TSESTree.Identifier`;
- `Property 'types' does not exist on type 'ts.Type'` (×2) — the
  `isUnion(): this is UnionType` narrowing, on a `Type` reached through
  `ReturnType<TypeChecker['getTypeAtLocation']>`;
- `Property 'program' does not exist on type 'Partial<ParserServices>'`;
- `Cannot find name 'ESLintUtils'` — `typeof ESLintUtils.RuleCreator` where
  `ESLintUtils` came from `import type`;
- `Property 'template' does not exist on type 'TemplateStringsArray'`.

Two of them — the `parent` pair — are now root-caused: they are the heritage
half of the relative-augmentation gap above, and reproduce in fifteen lines
once the augmentation is written with a relative specifier. The other three
still do not reproduce on a hand-written stand-in, so whatever breaks there
needs the real declarations.

## ts-pattern surge-only inventory (2026-09-11)

gvergnaud/ts-pattern 5.9.0 at `c92ca43`, installed with
`pnpm run real:ts-pattern:provision` (blobless clone, `npm ci --ignore-scripts`;
the repo ships `package-lock.json` and pins no package manager). Only the test
suite needs dependencies at all — `src/` has none — and the one that matters is
`@types/jest`.

The harness target is an aggregate, `tsconfig.surge.json`, kept in the repo at
`scripts/real-projects/targets/ts-pattern.tsconfig.json` and copied into the
corpus by the provisioning script. Neither upstream tsconfig can serve: the root
one covers `src/` only and emits declarations, and `tests/tsconfig.json` sets
`downlevelIteration`, which TypeScript 7 removed (the oracle answers `TS5102`
and checks nothing). The aggregate is 18 `src/` files plus the 48-file
type-level test suite in one program, `strict`, `target: ESNext`,
`moduleResolution: bundler`, `types: ["jest", "node"]`.

This is **not** a clean-oracle corpus. The pinned TypeScript 7.0.2 oracle
reports 2 diagnostics of its own, both `TS2344 Type 'false' does not satisfy
the constraint 'true'` — a `Expect<Equal<…>>` assertion the library wrote
against TypeScript 5.9 that the newer compiler evaluates to `false`, at
`tests/distribute-unions.test.ts:240:7` and `tests/when.test.ts:218:35`. surge
matches neither, so the corpus starts with 2 false negatives as well.

surge reported **446** on the first measurement, every one of them surge-only.
**431 of them were one root cause, now closed** — see below — and a second pass
closed twelve more, and a third closed six of the seven that were left. The
corpus stands at **1**. Every measurement here is from a **dirty working tree**
on top of `6e034fd`; this is a burn-down list, not a gate. The tree moved under
the measurements — another session was editing the same checkout, which is why
the first baseline reads 450 rather than 446 — so each figure is an A/B against
a baseline built minutes apart from the same tree with that pass's changes
reverted.

| Code | At 446 | Now | Where |
| --- | ---: | ---: | --- |
| TS2554 | 431 | **0** | 37 test files, led by `exhaustive-match` ×77, `variadic-tuples` ×49, `record` ×34, `types` ×31 |
| TS2345 | 8 | 8 | tests |
| TS2322 | 4 | 4 | 3 tests, 1 `src/patterns.ts` |
| TS2304 | 1 | 1 | `src/patterns.ts` |
| TS2339 | 1 | 1 | tests |
| TS2538 | 1 | 1 | `src/patterns.ts` |

### The 431: type-literal members were never grouped into overloads

Every `TS2554 Expected 3 arguments, but got 2` was a
`match(input).with(pattern, handler)` call. The cause is not in ts-pattern's
type machinery at all — it is that **`.with` was never an overload group**.

Same-named function members of an *interface* body were folded into one
permissive signature by `merge_overload_signatures`; the identical members
written in a *type literal* were not. `resolve_object_type` inserted each
property into the map in source order, so the last declaration won and every
earlier overload was silently dropped. `Match<i, o>` is a type alias whose
fourth and last `.with` overload takes three parameters, so a two-argument call
was checked against that one.

Seven lines reproduce it with no imports, and show that the interface spelling
of the same pair was already correct:

```ts
interface I {
  m<a>(x: a, y: (v: a) => void): number;
  m<a>(x: a, y: a, z: (v: a) => void): number;
}
declare const i: I;
i.m('a', (v) => {});      // fine

type T = {
  m<a>(x: a, y: (v: a) => void): number;
  m<a>(x: a, y: a, z: (v: a) => void): number;
};
declare const t: T;
t.m('a', (v) => {});      // TS2554 Expected 3 arguments, but got 2
```

The reduction went the other way from the obvious guess. The first overload's
`pattern` parameter is `IsNever<p> extends true ? Pattern<i> : p` over a
`const p extends Pattern<i>`, which looks like the suspect; replacing it with a
bare `p`, dropping `MatchedValue`/`FindSelected`, and finally dropping the
constraint entirely all left the diagnostic in place. A signature that trivially
applies still failed, which is what pointed at the group rather than the
signature.

A type literal now collapses a repeated function member the same way the
interface path does, pinned by the `type-literal-method-overloads-basic` preset.
The corpus went 446 → **15** with **no new diagnostic**: ky (0), ofetch (1),
zod (21), unnamed (0), trpc (1128) and the tanstack-query aggregate were
byte-identical across the change, and the full oracle preset sweep was unchanged
by it.

One limit is worth recording, because it is what the fixture can and cannot
pin. The collapse is permissive, not a real overload list: parameters come from
the longest signature, the required count is the smallest, and the return
degrades to the `Unknown` sentinel whenever two overloads disagree. For
non-generic overloads the return survives, so a merged `.read('a')` still types
as `string`. For *generic* ones — ts-pattern's `.with` among them — it degrades,
for interfaces and type literals alike. That is why the corpus lost 431 false
positives and gained nothing: the calls now resolve, but what they resolve to is
a degraded chain rather than a precise one. Closing that needs a real overload
list — the blocked overload program referenced above under `Object.fromEntries`,
not this fix.

### The twelve the second pass closed

| What closed | Count | Root cause | Preset |
| --- | ---: | --- | --- |
| `src/patterns.ts:104` | 1 | `Matchable[matcher]` — an indexed access keyed by a symbol. The parser drops computed members, so there was no member table to validate the key against and the report described surge's own gap. A symbol key now degrades silently, as an open receiver and an unresolved key already did. | `symbol-keyed-indexed-access-basic` |
| `src/patterns.ts:641`, `src/types/Pattern.ts:207` ×4 | 5 | `(value: any) => value is infer narrowed` — the `infer` capture sits in the *predicate*, which the capture collector never walked, so the name was not seeded and the true branch resolved it as an unknown type name (`TS2304`). It reproduces only in the full program; a standalone reduction stays clean, so nothing pins it. | (none) |
| `src/patterns.ts:700` | 1 | `typeof args[0] === 'string'` narrowed nothing: the tag test reached only bindings, not the per-access record the truthiness guards write, and the ternary path applies that narrowing separately from the statement path. The `unknown` keyword also carries no tag, so it survived the filter; in the matching branch the tag *is* its type. | `element-access-typeof-narrowing-basic` |
| `tests/tuples.test.ts` ×2, `tests/not.test.ts`, `intersection-and-union.test.ts` ×2 | 5 | An array literal against a union target took the union's *lone* array/tuple member and gave up when there were several, so `['-', 2]` widened to `(string \| number)[]`. The literal now picks the one member it fits, elements read unwidened. | `union-target-sequence-literal-basic` |

### The six the third pass closed

Measured 7 → **1**, with every other corpus byte-identical: ky (0), ofetch (1),
zod (21), unnamed (0), trpc (1128) and the tanstack-query aggregate (30) do not
move.

| What closed | Count | Root cause | Preset |
| --- | ---: | --- | --- |
| `intersection-and-union.test.ts:654`, `:662`, `real-world.test.ts:136`, `:153`, `:170` | 5 | An object literal against a union of object members that all declare the written properties was evaluated context-free, so its *nested* literals widened and every member then rejected it. The member is now picked by the discriminant where there is one — a property written as a primitive literal, read without any evaluation — and otherwise by typing one property against the union of what the candidates declare for it. | `union-target-object-literal-member-basic` |
| `exhaustive-match.test.ts:886` | 1 | `as const` was not applied on the inference path at all: the sketch forwarded through the assertion, so `[[x, x, x]] as const` arrived as `number[][]` and inferring `B` from `readonly B[]` gave `number[]`, or `any` through a callback. The assertion is now honoured recursively there, as it already was on the declaration and expected-type paths. | `const-assertion-nested-literal-basic` |

**The object-literal fix carries a measured cost bound.** Typing a literal
against a *wide* member is the expensive half — not the probe. With no bound, and
even with every probe removed so only the free discriminant test remained, the
tanstack-query aggregate went from about 4 s to not finishing in five minutes on
its query-option unions. A candidate with more than 20 own properties is
therefore left alone; a cap of 40 already reproduces the blowup. It is a cost
bound, not a semantic rule, and it is why the 11-member datadog union in
`real-world.test.ts` is reachable while tanstack's is not.

### The one that remains

| Location | Code | What it looks like |
| --- | --- | --- |
| `tests/is-matching.test.ts:161` | TS2339 | `isMatching({ someProperty: P.array() }, input)` narrows nothing, and reading `someProperty` on the un-narrowed union then reports against the arm that lacks it. The earlier reading of this line — that it narrowed to the *wrong* member — was wrong: the message only names the offending arm. Two layers sat behind it, and the first is now fixed; see below. |

**What the last one turned out to be: three layers, two of them now fixed.**
Neither of the two was about ts-pattern.

An overload group keeps one declaration's parsed signature, and guard narrowing
reads the type predicate off exactly that signature, so a predicate written on a
*later* overload was invisible. `isMatching` declares its predicate on the second
of three declarations, so the guard never found one. Ten lines with no imports
reproduce it. The predicate-bearing overload is now carried alongside the kept
signature in a field only narrowing reads — promoting it instead made the other
overload's callers report as not callable — pinned by
`overload-group-type-predicate-basic`.

A generic predicate's type arguments were then inferred from the *tested*
argument alone, so anything stated in terms of another argument
(`isOfKind(kind, value): value is Extract<T, { kind: K }>`) left that parameter
unbound and fell back to `any`. That is a false-positive class of its own —
`Extract<T, { kind: any }>` matches the wrong member and *both* branches report
— and it is fixed and pinned by
`predicate-type-argument-from-arguments-basic`.

With both, `isMatching`'s `P` binds to the pattern argument and the predicate
resolves instead of being abandoned. The corpus still does not move, because the
third layer is the blocked overload program: `P.array()` comes back as `any` from
its own overload group, so the pattern type is `{ someProperty: any }`, and
ts-pattern's `InvertPattern` on a degraded pattern yields `never` where tsc gets
`{ someProperty: never }`. The predicate becomes `T & never` per member and
narrows nothing. Two smaller gaps sit alongside it: `X & never` is not reduced to
`never`, and only the real `ExtractPreciseValue` chain decides the rest.

**Rejected on the way there: typing the literal against each union member
speculatively.** The obvious way to pick the member is to try each one and keep
the one that reports nothing. It was implemented and measured twice, and fails on
both axes that matter. tRPC gained **5 false TS2322** — a callback typed against
the wrong candidate reports against it, on `httpLink`, `httpSubscriptionLink`,
`loggerLink` and `observable` — and tanstack-query did not finish in **ten
minutes** where it takes five seconds, because each speculative attempt starts
more speculation inside itself. Restricting it to data-shaped literals with a
one-level depth guard still did not finish ts-pattern's own 0.2 s corpus. Reading
the discriminant instead costs nothing and decides the same cases; a real
speculative-checking transaction is what the general form would need. Recorded so
it is not re-proposed as an easy fix.

Both passes were timed interleaved against their own baseline, because the
narrowing and literal-selection paths are hot. Second pass: zod's median 3.22 s
before against 3.11 s after over five runs each, trpc 4.73 / 4.30 s against
4.46 / 4.31 s. Third pass: zod 3.56 s against 3.50 s, trpc 8.85 s against 8.91 s,
tanstack-query 5.25 s against 5.13 s, all under concurrent load that both
binaries saw. No regression at this resolution.

**Do not read a speed result out of this corpus.** surge finishes the aggregate
in about 0.2 s where the oracle takes about 6.7 s, but the calls that would
drive ts-pattern's deep conditional instantiation still resolve to a degraded
chain rather than being instantiated. The comparison becomes meaningful when the
merged signature stops degrading, not when the diagnostic count drops.

### The two `tsc`-only assertions (2026-09-13)

Both are `Expect<Equal<…>>` assertions ts-pattern wrote against TypeScript 5.9
that the pinned 7.0.2 oracle evaluates to `false`. They looked like two obscure
type-level cases; they were the top of a five-layer stack, and the bottom four
layers were gaps in surge that had nothing to do with ts-pattern:

1. **Variadic tuples were not modelled.** A tuple with a rest element
   (`[...a, ...b]`, `[infer head, ...infer tail]`, `[...path, k]`) lowered to
   `Unknown` in the parser, so every type-level list walk degraded on its first
   step. Now a `ParsedType::VariadicTuple`; spread operands with a known length
   splice into a fixed tuple, and a tuple `extends` pattern decides its branch by
   arity (`try_tuple_infer_match`), which is what lets a recursion terminate on
   `[]`. Committed as `ca4420b`.
2. **No contravariant inference.** A union check type against a signature
   pattern bound nothing, so `UnionToIntersection` — the idiom behind
   `IsUnion`, `UnionToTuple`, and most of the ecosystem — was always the
   sentinel. Candidates found in parameter positions now intersect, return
   positions union (`bind_union_signature_infer_captures`).
3. **An intersection of signatures inferred from its permissive fold.** The
   fold's return degrades whenever two overloads disagree, so
   `UnionToTuple`'s `extends (_: any) => infer elem` never bound. The
   intersection now keeps its operands as an overload group and inference reads
   the *last* one, as tsc does.
4. **`Equal<a, b>` was always `true`.** Both deferred conditionals resolved to
   the same sentinel. Two generic signatures whose returns test their own type
   parameter are now related by identity of the resolved parts
   (`deferred_conditional_identity`); a sentinel on either side still degrades.
5. **`TS2344` on a type reference's arguments did not exist at all** — only a
   call's explicit `keyof` arguments were checked. Now checked for primitive
   and literal operands, with the constraint resolved in the *declaring* module
   (resolving it in the consumer's scope reported every declaring-module
   sibling as an unknown name: zod +89 `TS2304`). Structural operands stay out
   because surge's structural gaps surface there as false positives
   (`PromiseLike<unknown>` against `WeakKey`).

Three general bugs were found under the same probes and are fixed with the
stack: **`1 extends object` was true** (the `object` keyword was the empty
object type, which primitives satisfy — now a `non_primitive` marker;
`IsPlainObject<1>` was `true` because of it); **`X extends unknown` always took
the false branch** (the genuine `unknown` constraint shared `is_unknown()` with
the degradation sentinel and was treated as unmodelled — `@bomb.sh/args`'
`UnionToIntersection` therefore resolved to `never`); and **a symbol-keyed
member vanished from its type literal** (`{ readonly [Symbol.toStringTag]:
string }` became `{}`, which everything satisfies — kept under a name no
identifier can spell). Also: tuple `['length']`, the empty tuple's element type
as `never`, and `never`-inference defaulting an `infer` capture to the genuine
`unknown`.

With all of it, `FindUnions<{ a: 1 | 2 }, { a: 1 }>` matches the oracle exactly
under `SURGE_GENERIC_RECURSIVE_ALIAS=1`, and the corpus **does not move** on the
default configuration (ky 0, ofetch 1, zod 21, trpc 1155, tanstack-query 10,
ts-pattern 1, all measured on a dirty tree). Under that gate ts-pattern's one
surge-only report disappears and the run completes (it used to die), at the cost
of one tanstack-query report; the default was not flipped here.

**Why the two assertions stay open.** Each rests on a 7.0.2 behaviour that is
not semantics surge can reason its way to:

- `distribute-unions.test.ts:240` — `FindUnions` now evaluates the whole
  `res5`, and the only difference from the oracle is **union member order**:
  surge yields `[a,e], [a,f], [b]`, which is what TypeScript 5.9 produced and
  what the test asserts; 7.0.2 yields `[b], [a,e], [a,f]`. surge's unions keep
  first-seen order; tsgo sorts constituents by internal type id. Reproducing
  that means reproducing tsgo's type-creation order, which is an implementation
  detail, not a rule.
- `when.test.ts:218` — the oracle types the `P.when((x) => …)` parameter as
  **`unknown` whenever the predicate sits inside `P.array({ … })`**, with or
  without a surrounding `.with`, and as `string` in a plain object pattern. That
  is 7.0.2 giving up contextual inference at that nesting where 5.9 did not.
  surge yields the sentinel there (the sealed overload program above), so
  `Equal` declines to decide. Matching the oracle means matching *where* its
  generic-call inference fails.

Neither is a fixture; both are recorded so the next reader does not re-derive
the stack. **The workspace test suite and the oracle sweep were not run for
this pass** — every attempt was killed by machine pressure from concurrent
builds — so the numbers above are corpus measurements only, not a gate result.

## drizzle-orm corpus (provisioned 2026-09-13)

drizzle-team/drizzle-orm `drizzle-orm@0.45.3` at `b786252`, installed with
`pnpm run real:drizzle-orm:provision` (blobless clone, `corepack pnpm install
--filter ./drizzle-orm --ignore-scripts`). Only the `drizzle-orm` package is
installed: `drizzle-kit`, `drizzle-seed` and `integration-tests` pull in database
servers and bundler toolchains the corpus never checks, while the package's own
devDependencies already carry every driver its sources import (`pg`, `mysql2`,
`better-sqlite3`, `@neondatabase/serverless`, `@planetscale/database`,
`@libsql/client`, `@electric-sql/pglite`, `gel`, `kysely`, `knex`, …), so the
filtered install is enough to type all of `src/`.

The harness target is an aggregate, `tsconfig.surge.json`, kept in the repo at
`scripts/real-projects/targets/drizzle-orm.tsconfig.json` and copied into the
corpus by the provisioning script. Unlike the tanstack-query and ts-pattern
targets it is written into the *package* directory, not the repo root, because
`paths` and `typeRoots` resolve relative to the tsconfig that declares them and
upstream's own type gate (`cd type-tests && tsc`) runs from there too.

No upstream tsconfig can serve directly: every one of them inherits `baseUrl`
from the repo root, which TypeScript 7 removed — the oracle answers `TS5102`
(plus `TS5090` on the non-relative `paths` value) and checks nothing. The
aggregate restates the root `compilerOptions` verbatim minus `baseUrl`, and
replaces it with the mapping TypeScript 7 suggests: `"~/*": ["./src/*"]` for the
package's own alias plus `"*": ["./*"]` for the root-relative imports the
type-tests write (`import { Equal } from 'type-tests/utils.ts'`). It is 448
`src/` files plus the 80-file `type-tests/` suite in one program — the same set
upstream's `test:types` covers — under the repo's own settings, which are
unusually strict: `strict`, `noUncheckedIndexedAccess`,
`noPropertyAccessFromIndexSignature`, `noImplicitOverride`, `noImplicitReturns`,
`exactOptionalPropertyTypes: false`, `checkJs`, `moduleResolution: bundler`,
`allowImportingTsExtensions`.

`src/prisma` is excluded. Those three drivers import the *generated* Prisma
client, which only exists after `prisma generate` — a codegen step that
downloads a query engine, so provisioning deliberately skips it. Left in, it
costs six diagnostics on both sides (`TS2305` ×3, `TS7006` ×3) that measure the
missing codegen rather than the checker. `typeRoots` and `types` are pinned to
the package's own `node_modules/@types` so the surrounding surge-ts workspace's
`@types/node` can never be picked up by the upward typeRoot walk.

This is **not** a clean-oracle corpus. The pinned TypeScript 7.0.2 oracle
reports 16 diagnostics of its own, all in `type-tests/`, all the same shape:
eight `@ts-expect-error` directives that drizzle wrote against TypeScript 5.6
and that TypeScript 7 no longer satisfies at that line, each producing a
`TS2578 Unused '@ts-expect-error' directive` plus the `TS2769 No overload matches
this call` that landed one construct over. surge matches none of them, so the
corpus starts with 16 false negatives as well.

### First measurement (2026-09-13)

surge reports **134**, every one of them surge-only, from a **dirty working
tree** on top of `6641008` (a copy of the tree's binary taken at 15:31, so this
is not a clean-worktree figure and is not carried into the status table). It
completes in about 2.4 s at about 480 MB peak RSS, against 12.8 s / 1.22 GB for
`tsc` and 2.6 s / 1.66 GB for `tsgo` on the same target. No hang, no unbounded
expansion: the corpus is usable as-is.

| Code | Count |
| --- | ---: |
| TS2339 property does not exist | 42 |
| TS18048 possibly 'undefined' | 27 |
| TS2304 cannot find name | 19 |
| TS2322 not assignable | 17 |
| TS2349 not callable | 11 |
| TS2351 not constructable | 7 |
| TS7006 implicit any parameter | 5 |
| TS2355 / TS2538 / TS2345 | 2 each |

What is identified so far:

- **drizzle's `is()` entity guard is the dominant cluster.** `src/entity.ts`
  declares `is<T extends DrizzleEntityClass<any>>(value: any, type: T): value is
  InstanceType<T>`, where `DrizzleEntityClass<T>` is
  `((abstract new (...args: any[]) => T) | (new (...args: any[]) => T)) &
  DrizzleEntity` — so `T` is inferred from a *class value* passed as the second
  argument and the narrowed type is `InstanceType<T>`. surge does not land on the
  instance type, and every dialect, `utils.ts`, `relations.ts` and migrator file
  in the repo is written in terms of this one guard. **70 of the 134 sit within
  eight lines of an `is(...)` call** — a proximity count, not an attribution;
  the sampled `TS2339` (`entry.decoder`, `table._.usedTables`, `relation.config`,
  `.dbMigrations`) and all 10 of the `PgDialect | PgDialectConfig` `TS2322`
  (`this.dialect = is(dialect, PgDialect) ? dialect : undefined`) are confirmed
  by reading the source. A minimal 18-line reproduction of the guard shape
  *passes* on surge, so an additional ingredient in the real shape is still
  unaccounted for. (It was the class being **generic**; see the burn-down pass
  below.)
- **`/// <reference types="…" />` is not honoured** — 19 `TS2304`, all of them
  ambient globals the referencing file pulls in that way: `D1Database`,
  `D1PreparedStatement`, `D1Response`, `D1Result` from
  `@cloudflare/workers-types` in `src/d1/`, and `DurableObjectStorage`,
  `SqlStorageCursor` in `src/durable-sqlite/`. Both packages are installed and
  neither is in `types`, which is exactly the case the directive exists for.
- **A parameter is not in scope inside its own type-predicate annotation** —
  `TS2304: Cannot find name 'c'` ×2, both
  `.filter((c): c is Exclude<typeof c, undefined> => c !== undefined)` in
  `src/sql/expressions/conditions.ts`.
- **`abstract` members are checked as if they had bodies** — the two `TS2355`
  in `src/cache/core/cache.ts` are `abstract strategy(): 'explicit' | 'all'` and
  its sibling, reported as "must return a value".
- **`T[string]` on a `Record`-shaped alias** — the one `TS2538` in
  `src/operations.ts:39` is `SelectedFieldsFlat<TColumn>[string]`.
- Not yet reduced: the 7 `TS2351 This expression is not constructable` on
  `new SQL(…)` / `new StringChunk(…)` (both classes carry a computed
  `static readonly [entityKind]: string`), the 11 `TS2349 This expression is not
  callable` on `drizzle(new Client())` (a function merged with a namespace,
  re-exported through the package entry), the 7 `Type 'Config | undefined' is not
  assignable to type 'string'` across `src/libsql/`, and the 10 `TS18048` in
  `src/aws-data-api/common/index.ts` on a repeated `field.arrayValue` access.

### First burn-down pass (2026-09-13): 134 → 82

**52 closed, nothing new, every other corpus byte-identical.** Measured from an
isolated worktree at `1978841` — the surrounding tree carried another session's
uncommitted edits, and an A/B across them produced a phantom trpc regression
that vanished once both binaries were built from the same tree. ky (0), zod
(23), ofetch (1), ts-pattern (1), tanstack-query (10) and trpc (1155) are
byte-identical before and after.

| Code | Before | After |
| --- | ---: | ---: |
| TS2339 property does not exist | 42 | 8 |
| TS18048 possibly 'undefined' | 27 | 11 |
| TS2304 cannot find name | 19 | 19 |
| TS2322 not assignable | 17 | 17 |
| TS2349 not callable | 11 | 11 |
| TS2351 not constructable | 7 | 7 |
| TS7006 implicit any parameter | 5 | 5 |
| TS2355 / TS2538 / TS2345 | 2 / 2 / 2 | 2 / 2 / 0 |

What closed, and why it was one thing:

- **`is()`'s type argument came from a class value, and a generic class's value
  side is `any`.** `build_class_value_symbol_with_scope` deliberately models a
  generic class's value as `any` — a real static object over it was measured on
  2026-09-12 to open TS2351/TS2554 across zod, trpc and ofetch, and that
  measurement is not reopened here. But the identity that `any` erases is the
  entire content of `is<T extends DrizzleEntityClass<any>>(value: any, type: T):
  value is InstanceType<T>`: `T` binds to the *class*, and the predicate is
  `InstanceType<T>`. With `T` bound to `any`, `InstanceType<any>` came back
  `any`, and the guard then did one of two wrong things — inside a function body
  it replaced the subject with `any`, silently swallowing every later error in
  the branch, and in a conditional expression it proved nothing and left the
  whole union standing, which is where the 42 `TS2339` came from. Predicate
  type-argument inference now stands a constructor surface (`{ prototype:
  Instance }` plus a construct signature returning `Instance`) over the class
  the argument names, with each of the class's type parameters filled by `any`.
  It runs for inference only; the value's own type is untouched. Pinned by
  `generic-class-entity-guard-basic`.
- **The same gap reached a namespace-merged class from the other side.** `class
  SQL` merged with `namespace SQL { class Aliased }` has a *value*: the
  namespace object. It carries `Aliased` but no construct signature, so `T`
  bound to a shape `InstanceType` could not peel either. The recovery is gated
  on "no construct signature" rather than on `any` for that reason, and the
  argument expression is read as a dotted path so `is(entry, SQL.Aliased)`
  resolves the namespace member.
- **Closing it exposed one new over-report, closed in the same pass.** `readonly
  encoder: DriverValueEncoder<…> = noopEncoder` in `Param`'s constructor is a
  parameter property with a default. The parser folds a default into the
  parameter's `optional` flag so call arity accepts the omitted argument, and
  the class's instance side read that same flag — so the *property* came out
  optional and `p.encoder.mapToDriverValue(…)` reported `TS18048`. It had been
  invisible because `is(p, Param)` narrowed to `any` and hid it. A parameter
  property is optional only when written with `?`. Pinned by
  `parameter-property-default-required-basic`.

The minimal reproduction is three lines of declaration and one of use: a
*generic* class passed by value to a guard whose predicate is `InstanceType<T>`.
The same shape with a non-generic class was always correct, which is what made
the first several reductions come back clean — the class in the probe has to be
generic.

The 82 that remain are the inventory above minus the `TS2339` and `TS18048` the
guard was producing. The next levers, in size order, are the 19 `TS2304` from
unhonoured `/// <reference types="…" />`, the 17 `TS2322` (7 of them one
`Config | undefined` shape across `src/libsql/`), and the 11 `TS2349` on
`drizzle(new Client())`.

### Second burn-down pass (2026-09-13): 82 → 49

Five causes, **33 closed, nothing new**, every other corpus byte-identical
throughout. Same isolated worktree at `1978841`.

| Cause | Closed | Preset |
| --- | ---: | --- |
| A type directive resolved to a package's `index.ts`, not its `index.d.ts` | 17 | `reference-types-declaration-sibling-basic` |
| A union member a property-path guard rules out was kept | 10 | `property-path-impossible-union-member-basic` |
| A bodyless class member's absent body was checked | 2 | `abstract-member-no-body-basic` |
| `T[string]` rejected instead of reading the index signature | 2 | `string-keyword-indexed-access-basic` |
| The `filter`-arrow predicate query reported out of scope | 2 | `filter-arrow-predicate-self-reference-basic` |

- **`/// <reference types="pkg" />` took the source flavor.** An extensionless
  entrypoint probes `.ts` before `.d.ts`, which is right for *module* resolution;
  a type reference directive is resolved with `Extensions.Declaration` and never
  reaches a `.ts`. `@cloudflare/workers-types` ships both flavors of one surface
  — `declare abstract class D1Database` in `index.d.ts`, a script, so the class
  is a global, and the same declaration with `export` in `index.ts`, a module,
  where it is not. Taking the source resolved the directive, reported no
  `TS2688`, and contributed no globals at all. That was every remaining
  `TS2304` bar two.
- **A union member the guard rules out was kept.** The leaf narrowers answer
  `None` both for "nothing to narrow" and for "this member cannot satisfy the
  guard at all", and the union walk read both as "keep it". Every member of the
  AWS SDK's `Field` declares the other members' keys as `?: never`, so
  `field.arrayValue !== undefined` leaves exactly one member possible — but at a
  `?: never` leaf the effective `never | undefined` collapses to plain
  `undefined`, which has no union to split. Ten `TS18048` in one function.
- **A bodyless class member was run through the function-body check**, which
  reported the missing return of a body that is not there. The identical guard
  had always been on the function-declaration path; class members never had it.
  Fixing it exposed that `ParsedClassMethod` carried no return-type span either,
  so a class method's `TS2355` always landed on the name where tsc puts it on
  the written return type; both are in the same commit.
- **`T[string]` was rejected outright.** The `string` keyword as an index reads
  the receiver's string index signature — `Record<string, V>[string]` is `V`,
  the normal way to name a record's value type. A receiver *without* an index
  signature is left degrading silently: tsc answers `TS2537` there, which surge
  does not have.
- **The `filter`-arrow predicate query reported.** Reading an inline
  `(c): c is T =>` arrow's target to narrow the filtered element type resolved
  the written type with neither the arrow's parameters in scope nor diagnostics
  dropped, so `(c): c is Exclude<typeof c, undefined>` reported a false
  `TS2304` on `c`. Only the reporting is dropped; an unresolvable target still
  yields no narrowing.

**One cluster was investigated and abandoned.** The 10 `TS2322` in the five
`*-core/query-builders/query-builder.ts` files (`is(dialect, PgDialect) ?
dialect : undefined`) are *not* a guard problem: `PgDialect`'s own instance
resolution comes back `had_error`, so its static object is refused as a
type-argument candidate (`type_argument_is_unresolved`) and the guard is
abandoned before it narrows. Extending the entity-guard recovery to fire on a
degraded value was implemented and **rejected by measurement** — drizzle went
65 → 67. What degrades inside `PgDialect` is a separate investigation.

The 49 that remain: `TS2322` ×17 (the 10 above plus 7 `Config | undefined` in
`src/libsql/`), `TS2349` ×11 on `drizzle(new Client())` — a function merged with
a namespace, re-exported through the package entry — `TS2339` ×8, `TS2351` ×7 on
`new SQL(…)`, `TS7006` ×5, `TS18048` ×1.


## zustand corpus (provisioned 2026-09-13)

pmndrs/zustand `v5.0.15` at `2115efb`, installed with
`pnpm run real:zustand:provision` (blobless clone, `corepack pnpm install
--ignore-scripts`). Every dependency the corpus types is a devDependency of the
one package — `react`, `@types/react`, `immer`, `redux`,
`@redux-devtools/extension`, `use-sync-external-store`, `@testing-library/*` —
so a plain install is enough.

**There is no aggregate target.** Upstream's root `tsconfig.json` *is* the
corpus: `pnpm test:types` runs `tsc --noEmit` against it, it covers `src/` and
`tests/` in one program, and TypeScript 7 needs nothing removed from it. That
makes zustand the only corpus measured exactly as it ships. It is 31 root files
(30 `.ts`/`.tsx` plus `src/types.d.ts`) under `strict`,
`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`,
`verbatimModuleSyntax`, `isolatedDeclarations`, `allowImportingTsExtensions`,
`jsx: react-jsx`, `moduleResolution: bundler`, with `paths` mapping `zustand`
and `zustand/*` onto `./src/`.

The pinned TypeScript 7.0.2 oracle reports **0** diagnostics, so — like ky and
unnamed — every surge diagnostic on it is a false positive. surge reported
**136** on the first measurement (2026-09-13, clean worktree at `31d9bd6b`),
all surge-only, and finished in about 0.24 s at about 88 MB peak RSS (`tsc`
1.71 s / 326 MB, `tsgo` 0.19 s / 181 MB on the same target): the smallest and
cheapest corpus in the set, and the fastest to iterate on. It is deliberately
**not** a gate at 136; a ky-style exact-0/0 assertion can be armed when it
reaches zero.

First-measurement inventory, by code: `TS2339` ×79, `TS2349` ×34, `TS2304` ×9,
`TS2322` ×4, `TS2345` ×4, `TS18048` ×3, `TS2536` ×1, `TS2353` ×1, `TS7006` ×1.
By file the weight is in `tests/basic.test.tsx` (39), `tests/persistAsync.test.tsx`
(26), `tests/middlewareTypes.test.tsx` (23) and `tests/shallow.test.tsx` (9); the
`src/` side is 21.

Three causes are reduced, and the first two account for the nine `TS2304`
directly. None is pinned by a fixture — a fixture here would bake in the false
positive rather than a fix.

- **An augmented interface's members resolve in the augmented file's scope.**
  zustand's whole middleware typing rests on one mechanism: each middleware
  declares `declare module 'zustand/vanilla' { interface StoreMutators<S, A> {
  'zustand/persist': WithPersist<S> } }`, and `Mutate<S, Ms>` in
  `src/vanilla.ts` indexes `StoreMutators` to compose the store type. surge
  merges the augmentation member but resolves its *type* where the interface was
  declared, so `WithDevtools`, `WithPersist` and `WithRedux` — all local to their
  own middleware file — are `TS2304` at `src/vanilla.ts:16`, `:25` and `:101`,
  and the store type they feed loses its middleware surface. That loss is the
  likely engine behind most of the 79 `TS2339` and 34 `TS2349`: the test files
  call `store.persist.*`, `store.setState(…, action)` and `useStore(…)` on a
  store whose mutators never applied. Reduced to two files of a dozen lines
  (`export interface StoreMutators<S, A> {}` plus `Mutate` indexing it; a
  sibling that augments it with a file-local alias) — tsc is clean and surge
  reports `TS2304` at the *declaring* file's member span. Adjacent to the
  augmentation-visibility gaps inventoried for tanstack-query, where an
  augmentation of a base interface is not seen from a derived one.
- **`infer` names past the first signature of an overloaded call-signature
  group are unbound.** `persist.ts:125` and `devtools.ts:68` both write
  `S extends { setState: { (...args: infer Sa1): infer Sr1; (...args: infer
  Sa2): infer Sr2 } }` to capture both `setState` overloads, and the true branch
  then names all four. `Sr1` resolves; `Sa1`, `Sa2` and `Sr2` are `TS2304`, so
  only the first signature's return is registered from the group. Reduced to
  nine lines with no imports; the single-signature form
  (`{ getState: () => infer T }`, which `ExtractState` uses) is fine.
- **A captured `let` is not narrowed past its last assignment.**
  `createJSONStorage` declares `let storage: StateStorage<R> | undefined`,
  assigns it inside a `try` whose `catch` returns, and then reads it from the
  callbacks of the object literal it returns. Nothing assigns it after those
  callbacks are created, so tsc narrows the capture to the assigned type; surge
  reads the declared type and answers `TS18048` ×3 (`persist.ts:50`, `:57`,
  `:58`). Reduced to thirteen lines with no imports.

Three smaller items are not yet reduced. `src/vanilla/shallow.ts` carries four
`TS2339` where `valueA instanceof Map ? valueA : new Map(valueA.entries())`
does not come out as a `Map` — at `:18` the result is the parameter's own shape
alone and at `:22` a union of that shape with `Map<any, any>`, so the two
branches are being combined differently on the two lines. `src/vanilla.ts:20`
answers `TS2536` on `number extends Ms['length' & keyof Ms]`, the
intersection-keyed index zustand uses to test for a non-tuple array.
`src/middleware/immer.ts:76` answers one `TS7006` on the rest parameter of
`store.setState = (updater, replace, ...args) => …`, contextually typed by the
assignment target's overloaded `setState`.


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

`unnamed` is a real-project compatibility target: a local
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
The oracle preset sweep is **76/76** (75 prior + the new fixture) under the normal
gate.

### Remaining next recommended fix

Type-only re-export of a namespace value (`import type { z } from "zod"` in the 3
`*-form.tsx` files, plus the exported-type `Locale` whose RHS
`(typeof routing.locales)[number]` is unresolved) — the type-side analogue of the
value re-export fixed here. After that, the dominant remaining drift
(TS7031/TS7006 React contextual callback inference) is the next high-impact but
much larger area; it should not be attempted as a "small blocker".

The version-tagged milestone notes that used to follow here — the `v0.60`–`v0.85`
milestone log — have moved to
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

The version-tagged milestone notes (the `v0.60`–`v0.85` milestone log and the
per-feature `v0.7x`/`v0.8x` notes) and the
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

## Generic recursive aliases resolve to the degradation sentinel (2026-09-11)

The 62-diagnostic "tRPC's own router/client proxy types" row above is gated on
one modelling gap, and this section records it, its minimal reproduction, and
why the repair is opt-in.

### The gap

A resolution frame is keyed by `DeclarationResolutionKey`, which carries
`(file_name, name, namespace, fingerprint)` and **no type arguments**
(`fingerprint` is 0 for an ordinary declaration). A generic alias that
instantiates itself therefore collides with its own frame: `Decorate<{post: …}>`
and the `Decorate<{listPosts: …}>` its own body asks for are read as the same
declaration, so the inner one is treated as a self-cycle. `resolve_type_alias`
answers a *generic* back-edge with `Type::Unknown` — deliberately, to keep a
fluent builder's bounded peel from over-reporting — and every read downstream of
the value goes silent.

Reduced to six lines, with no conditional, no `infer` and no `Record` check
involved:

```ts
type M<T> = { [K in keyof T]: true extends true ? M<T[K]> : never };
declare const m: M<{ post: { listPosts: { query(): Promise<Post[]> } } }>;
const x: number = m.post;   // tsc: TS2322. surge: silent — `m.post` is open.
```

`tests/compat-projects/generic-record-proxy-indexed-access-basic` is the same
shape carried through to the diagnostic the corpus loses: a `TS2532` on
`posts[0].title` under `noUncheckedIndexedAccess`. It is **not** an indexed-access
or `noUncheckedIndexedAccess` bug; both are exact already.

### The repair, and why it is off by default

`SURGE_GENERIC_RECURSIVE_ALIAS=1` discriminates the `resolving` frame by the
instantiation's *resolved arguments* (`instantiation_frame_key`), so a
terminating recursion resolves concretely and only a repeated argument tuple is
a cycle, bounded by `MAX_NESTED_INSTANTIATIONS` and a combined
resolution/peel depth. With it on, the reproduction's whole chain types as tsc
types it — `client.post` is `Decorate<{listPosts: …}>`, `.query()` returns
`Promise<Post[]>`, and the `TS2532` appears on its own.

Two things were measured and are worth keeping:

- Handing a generic back-edge a **lazy self-reference** (the non-generic
  treatment) instead of the sentinel overflows the stack on the tRPC aggregate.
  The crash is in `merge_intersection_members_now` peeling a self-referential
  `LazyInstantiation` — the intersection self-reference class. Only the frame
  identity may change; the back-edge itself must stay the sentinel.
- Extending the same frame identity to `resolve_interface` was tried and
  abandoned: the run was killed on the tRPC aggregate. Interfaces are the large
  mutually-recursive clusters and need their own evidence.

**Measured on a dirty tree** (this repository was being edited by another
session throughout), tRPC checkout `dfbafa8`, TypeScript 7.0.2 oracle:

| corpus | gate off | gate on |
| --- | --- | --- |
| trpc | surge-only 0, tsc-only 116 | surge-only 0, tsc-only 116 |
| zod / ky / ofetch | exact match | byte-identical |
| tanstack-query | surge-only 32 | surge-only **34** |

So the gate closes the isolated gap and costs two tanstack-query false
positives, which is why it stays off. Both are `TS2322` in
`query-core/src/__tests__/utils.test-d.tsx`, where a recursive tuple-prefix
union loses its recursive member and the expected type narrows from the prefix
union to the full tuple (`Type 'string[]' is not assignable to type
'["key"] | ["key", "something"]'`). The union resolver propagates `had_error`
correctly and `union_type` drops only `Never`, so the recursive member is
collapsing to `never` before the union sees it — that is the next thing to
reduce.

It does **not** move tRPC on its own, and it does not materially change the
analysis-scope trade: with `SURGE_UPGRADE_ANALYSIS_SCOPES=1` the `tsc`-only
side is 104 either way and the surge-only side goes 15 → 14. The router/client
cluster needs the interface half as well.

Counts in this section are diagnostics (the `By file/code/line` basis), the
same basis as the 116 in the inventory above — not the coarser `By file/code`
aggregation, which collapses several lines of one code in one file and reads
low.

### Where degradation provenance is lost

Three sites were confirmed while reducing the above. They explain why the
earlier per-name and whole-substitution provenance experiments never reached
`ProtectedIntersection`'s conditional:

1. `resolve/named.rs` — a substitution hit returns `had_error: false`
   unconditionally, so a degraded binding is laundered clean.
2. `resolve/mod.rs`, the `keyof` arm — **every** exit path returns
   `had_error: false`, including the `_ =>` fallback that a degraded operand
   always lands in. This is the one that matters: even with perfect
   substitution provenance, `keyof <degraded>` hands back a clean `Unknown` one
   step later, the intersection simplifies it away, and `extends never` answers
   from a key set that never existed.
3. `resolve/substitution.rs` — the `pre_resolved` fast path in
   `bind_type_arguments` contributes no taint at all.

The conditional resolver's refusal guard (`resolved_check.had_error`) is already
correct, as are the intersection, indexed-access and mapped-type propagations.
The signal simply never arrives.

### Degradation provenance lands, and the scope gate becomes clean (2026-09-12)

The three laundering sites above are fixed, and the consequence is the one the
section predicted: `SURGE_UPGRADE_ANALYSIS_SCOPES=1` no longer costs a single
false positive on tRPC.

- `TypeParameterSubstitution` carries a `degraded` name set beside its
  bindings, in the same shape as `placeholders`, so the hot per-binding tuple
  does not grow and the common case costs one `None`. `bind_type_arguments`
  marks a parameter whose argument or default resolved with `had_error`, and
  the substitution hit in `resolve_named_type_inner` reports it.
- The `keyof` arm propagates its operand's taint instead of returning
  `had_error: false` on every path.
- `noImplicitReturns` treats `any` and the degradation sentinel as void-like.
  tsc asks its TS7030 question only of a non-`void`, non-`any` function, so
  `return <any>` suppresses it while `return <unknown>` does not; surge
  reported both. The same test decides it for a return expression surge could
  not model, which is what drew a TS7030 on `WsConnection.open`. Pinned by
  `implicit-returns-any-return-basic`.

| trpc | surge-only | tsc-only |
| --- | --- | --- |
| default | 0 | 116 |
| `SURGE_UPGRADE_ANALYSIS_SCOPES=1` | **0** (was 15) | 116 |
| both gates | **0** (was 14) | 116 |

The gate is now *safe* but not yet *productive*: the 12 it used to close were
being closed by accident. With a router surge cannot model, the collision
branch fired on a key set that was never there, and some of those wrong
diagnostics happened to land on the lines tsc reports. Now the conditional
refuses to choose and emits nothing, which is correct and leaves the count at
116. Making the gate close them for the right reason needs the router record to
resolve.

**The interface half failed a second time, with measurements.** Extending the
instantiation-aware frame identity to `resolve_interface` — restricted to
non-library interfaces, so the DOM and typed-array clusters are excluded — took
zod from 2.6s to a 300s timeout and collapsed tRPC from 1,128 diagnostics to 1.
The alias path terminates because an alias body is one type expression; an
interface body is a member table whose every member can re-enter the
declaration under a fresh argument tuple, so distinct-argument recursion has no
natural floor there. A future attempt needs a different termination argument,
not a bigger cap.

Not measurable at this commit: tanstack-query. It runs in 4s on the binary this
session started from and does not finish in 300s on the current tree, with both
provenance changes switched off, so the regression is in other work in this
tree and not in the changes described here. Record *not measured* for it.

### The tanstack-query "hang" was a measurement artifact (2026-09-12)

Recorded because it cost a full investigation. tanstack-query was reported above
as *not measurable* — 300s+ where the session's first binary took 4s. It runs in
**4.1s**, deterministically, and always did.

`target/release/surge` is shared, and another session was rebuilding it
throughout. The timings were taken against whatever binary happened to be on
disk at that moment, including half-written and older ones; the giveaway was the
binary's mtime moving three minutes past the build that produced it. A second
false alarm in the same investigation came from counting `len(json)` on a
diagnostics *object*, which counts keys, not diagnostics, and read 30 as 1.

Measure from a copy taken immediately after the build, and point the harness at
it with `SURGE_TS_BIN`, which also stops `oracle:compare` from rebuilding mid-run
through its default `cargo run`.

### Variadic tuple patterns are unmodelled, and that is what the gate exposes

With `SURGE_GENERIC_RECURSIVE_ALIAS=1` the recursive alias resolves, and
tanstack-query gains exactly one false positive: a `TS2322` in
`query-core/src/__tests__/utils.test-d.tsx` where `QueryFilters`' partial query
key narrows from the prefix union to the full tuple. Reduced, the cause is not
the gate:

| written pattern | tsc | surge |
| --- | --- | --- |
| `[...infer R]` | `["a","b"]` | `["a","b"]` |
| `[infer A, infer B]` | binds | nothing |
| `readonly [infer A, infer B]` | binds | nothing |
| `ReadonlyArray<infer E>` | binds | nothing |
| `[unknown, ...infer R]` | `["b"]` | nothing |
| `[...infer R, unknown]` | `["a"]` | nothing |

Two separate gaps sit behind that table. `ParsedType` has no rest element, so
`parse_tuple_type` sends any tuple containing one through
`homogeneous_variadic_tuple`, which can only express the homogeneous case and
otherwise yields `ParsedType::Unknown`. And `bind_infer_captures` has no
`ParsedType::Tuple` arm at all, so even a rest-free `[infer A, infer B]` binds
nothing. Only the degenerate `[...infer R]` survives, because it parses to the
spread operand itself.

Degrading around the first gap was tried and reverted; see
`undecidable-conditional-never-branch-basic`. Representing rest elements is the
repair, and it subsumes both gaps.

## tanstack-query react-query burn-down (2026-09-13)

The `react-query` type tests held eleven of the aggregate's twenty-eight false
positives. Ten are closed, by four root causes, and the aggregate went **28 →
18**. ky (0/0), unnamed (0/0), ofetch (1/1), zod (21/21), ts-pattern (1) and
trpc (1128, surge-only 0) are byte-identical before and after, and the preset
sweep is 209/209 on the normal gate. Measured on a dirty working tree on top of
`7780246`, TypeScript 7.0.2 oracle, against a binary copied straight after the
build and pointed at with `SURGE_TS_BIN`.

### A generic overload group discarded its parameter fold at instantiation

Six of the ten. An overload group's value type is the permissive fold of every
overload — a position declared differently across them becomes the union of what
they accept — and `register_function_signature` has built that union since the
`cacheLife` fix. A *generic* group never saw it. `instantiate_function_type`
rebuilds every parameter from `function_signature.parameter_types`, and a group
keeps exactly one parsed signature (the first declaration's), so the union was
overwritten by the first overload's shape at every call.

`useQuery({ queryKey, queryFn })` is the shape: the first overload takes
`DefinedInitialDataOptions`, which requires `initialData`, so the call reported
`TS2345` against a parameter type the call had never been meant to match — and
reported it with the type parameters unsubstituted, because inference against
the wrong shape found nothing. `useInfiniteQuery` and `mutationOptions` are the
same cause.

The group now carries its later overloads (`overload_alternatives`, bounded at
six, the way `predicate_overload` is already carried), and each is instantiated
against the same call with its own inference and its parameter unioned back in.
Diagnostics raised while resolving an overload the call did not pick are
discarded: a constraint violation in an unrelated overload is not the call's
error.

Parameters only, at first. The kept signature's return type survived the fold,
because widening it to the group's union — or to `any`, the way the non-generic
fold does — would degrade every generic group's result, and picking the matching
overload's return was the blocked overload-resolution program. That left the
eleventh false positive open: `useQuery<string, Error>(...)` was typed by the
*first* overload's `DefinedUseQueryResult`, whose two members are the
refetch-error and success results, so `state.isLoadingError` was `false` in both,
narrowing on it collapsed, and `state.error` read as possibly `undefined` where
tsc, having picked the second overload's five-member `QueryObserverResult`, has
`Error`. Overload return selection, below, closed it the same day.

Pinned by `overload-group-generic-parameter-fold-basic` and five tests in
`crates/surge-ts-checker/tests/function_overloads.rs`.

### TS2356 was applied to unary `+`/`-`, which coerce

`TS2356` ("An arithmetic operand must be of type 'any', 'number', 'bigint' or an
enum type") is the `++`/`--` operand rule. Unary `+`/`-` coerce: tsc accepts any
operand and types the result `number`. surge reported every non-numeric operand
and returned `unknown` for the result on top of it, which silently disabled the
downstream checks too.

Measured against the oracle, `+s` / `-s` / `~s` on a `string`, `+o` on an
`object` and `+b` on a `boolean` are all accepted by tsc; `s++`, `s--` and `++s`
report `TS2356`; and `+(u as unknown)` reports `TS2571`. surge had the rule
exactly inverted — it reported the accepted forms and reports nothing for the
three `++`/`--` forms tsc rejects. **Only the false-positive half is closed
here.** The `++`/`--` half and the `unknown` operand are new checks with their
own false-positive surface and are recorded, not smuggled in.

`useQueries.test-d.tsx` writes `(data) => [data, +data]` against a
`(data: string) => [string, number]` contextual type, which is the corpus hit.
Pinned by `unary-arithmetic-coercion-basic` and
`crates/surge-ts-checker/tests/unary_arithmetic_operand.rs`.

Seven smoke fixtures and one span test encoded the old behavior. Each was
re-verified against the pinned oracle on its own — `-"hello"`, `+true`,
`const value: number = -"hello"`, `+value` on a `string | number`, and the two
literal cases all report nothing in tsc — then renamed from `…-invalid` to
`…-coerces` and set to expect nothing. (One of them turned out to be listed
twice in the smoke manifest, under the same name and path.) `TS2356` has no
emission path left, so the catalog and the emitted-diagnostics manifest now
carry it as `catalog-only` with the reason attached; the `++`/`--` rule needs a
parser node for update expressions, which surge does not have.

### A declaration's annotation was re-checked under the call's substitution

A generic call re-resolves the callee's written annotations under its
substitution. That is not a fresh declaration check — the declaration was
checked where it was written, against the type parameter's *constraint* — so
anything raised during re-resolution lands on the declaration's span from a call
site that can neither see nor fix it.

`useWrappedQuery`'s `fetcher: (obj: TQueryKey[1], …)` under
`TQueryKey extends [string, Record<string, unknown>?]` is the corpus hit: called
with `['']`, the re-resolution read `TQueryKey` as `[""]` and reported a false
`TS2493` on the arrow's parameter list. tsc does not check a type parameter's
indexed access against the arity of its tuple constraint at all — `K[1]` under
`K extends [string]` resolves rather than reporting — so the suppression costs
nothing. A written out-of-range index on a *concrete* tuple is unaffected.

`instantiate_function_type_with_substitution` now discards what it raises.
Pinned by `instantiated-annotation-span-basic` and
`crates/surge-ts-checker/tests/instantiation_annotation_diagnostics.rs`.

### A missing-property report was made on a literal that could not be compared

tsc reports **one** error per object literal: a written property that fails is
reported at that property, and the missing-required-property report never
happens. surge already had that order. The gap was the degraded case — when the
expected member is surge's degradation sentinel the comparison passes
permissively, so the missing-property report fired *instead of* the property
error tsc reports.

Which matters beyond the message: `useSuspenseInfiniteQuery.test-d.tsx` writes
`@ts-expect-error` over the `queryFn: skipToken` property, covering the
property's line and not the literal's. tsc's single error is the property's and
is suppressed; surge's was the literal's `TS2741` for the missing
`initialPageParam`, two lines up and unsuppressed. The member behind it is
`queryFn?: Exclude<UseInfiniteQueryOptions<…>['queryFn'], SkipToken>`, which
surge resolves to the sentinel.

The report is now withheld when a property the literal writes was compared
against a sentinel member. The check is deliberately **shallow** — sentinel at
the top level of the member type, or of its optionality union. The deep walk
(`type_contains_degradation_sentinel`) resolves lazy references, and calling it
per property of every checked literal hung the tanstack aggregate: ten minutes
at 0% CPU and 151 MB, a deadlock rather than a slowdown, since the resolution
re-enters while the literal is mid-check.

No preset: the premise is a surge-internal degradation, so a project pinning it
would be green for the wrong reason, and would flip the moment `Exclude` over
that member resolves. `crates/surge-ts-checker/tests/object_literal_report_order.rs`
pins the ordering rule the fix depends on instead.

## Overload return selection (2026-09-13)

The overload-resolution program had been blocked since 2026-07-17 on a measured
regression: the `recovery/overload-only` branch tried each overload as a full
call and rolled its diagnostics back, which re-evaluated every argument per
candidate and re-inferred arguments on the inference path — **+79% user CPU and
+42% peak footprint on tRPC**. That branch is one commit, 294 commits behind
main, and was not rebased. The selection half of it is re-done here on a design
that adds no argument evaluation.

**The arguments are checked once, against the permissive fold, exactly as
before. With their evaluated types in hand, the return type is the first
overload's that accepts them, in declaration order; when none does, the fold's
return stays, so a no-match call reports exactly what it did before.** A group
carries its members on the `FunctionType` *handle* — `overloads`, next to the
display-only metadata — never on the payload, so identity, interning and
equality are unchanged. It is a thin `Arc<Vec<_>>` on purpose: a fat pointer
would have grown every `Type` from 96 to 104 bytes; the handle grows 88 → 96 and
`Type` stays at 96. Non-generic groups attach the list where the fold is built
(`merge_overload_group_signatures`); generic groups attach the per-call
instantiations the parameter fold already computes.

Selection is stricter than the fold's assignability where tsc is, because a
wrong pick is worse than no pick — the memo's own warning, "selection precision
is bounded by assignability precision", bit on the first run:

- **Weak types.** `fs.readFileSync(path, 'utf8')` picked the `Buffer` overload
  on both zod (+2) and trpc (+3), because surge's assignability lets `'utf8'`
  pass against `{ encoding?: null; flag?: string } | null`. tsc's weak-type rule
  is applied to selection only: an object type whose properties are all
  optional accepts nothing that shares no property with it. Assignability
  proper is untouched.
- **Written keys.** The tanstack call's option literal has a member surge
  cannot model (`queryFn`'s `Exclude` over `SkipToken`), so its evaluated type
  is a wildcard — and a wildcard would have accepted the `initialData` overload
  first. The property names an object literal writes are known from the syntax
  alone, so an overload requiring a property the literal never writes is
  rejected before its type is consulted.
- **Callbacks** are wildcards: typed by whichever overload is picked, so they
  cannot pick. **Spread** disables selection for the call.
- A parameter standing at the degradation sentinel rejects its candidate:
  committing to it would hand an equally degraded return to every consumer.

Measured, interleaved pairs against the pre-change binary, dirty tree on
`7780246`:

| corpus | diagnostics | user CPU (3 pairs) | peak RSS |
| --- | ---: | --- | --- |
| tanstack-query | 18 → **17** | 0.56 / 0.56 s | — |
| trpc | 1128, byte-identical | 5.38 / 5.42 s | 932 → 928 MB |
| zod | 21, byte-identical | 2.40 / 2.41 s | 388/389, 391/384, 382/370 MB (3 pairs, noise) |
| ky / ofetch / ts-pattern / unnamed | byte-identical | | |

Not done, deliberately: **`TS2769` is still not emitted**, so a call matching no
overload keeps its fold-shaped `TS2345`/`TS2322` and a preset cannot contain a
failing overloaded call. **Interface and type-literal method groups** still
resolve through the fold alone — the group list rides free-function and ambient
module-function groups only, which is where `register_function_signature` folds;
attaching lists to every `.d.ts` interface member was the other half of the old
branch's cost and needs its own measurement. **Expression inference**
(`infer_expression` on a call) reads the fold's return without selecting; a
binding still sees the selected type because declarations are typed through the
check path.

Pinned by `overload-return-selection-basic` and seven tests in
`crates/surge-ts-checker/tests/function_overloads.rs`.

## query-core: type-parameter candidates from every argument (2026-09-13)

tanstack-query **17 → 15**, every other corpus byte-identical, sweep 211/211.
`shallowEqualObjects({ a: 1 }, { a: 2 })` reported `Type '2' is not assignable
to type '1'`, and the `{ a: 1, b: 2 }` variant an excess property — reduced to
three lines: `declare function eq<T extends Record<string, any>>(a: T, b: T |
undefined)`.

Three layers, each pinned by `generic-object-candidate-union-basic` and
`crates/surge-ts-checker/tests/generic_literal_widening.rs`:

- **Later object candidates were dropped.** `record_type_argument_candidate`
  kept the first candidate and could only meet two *primitives* at their base
  type; two object shapes fell through, so `T` was fixed from the first
  argument. tsc infers the union of object-literal candidates; so does surge
  now.
- **A union parameter never reached the naked member.** The union arm of
  `collect_inferred_type_argument` returned as soon as an argument matched none
  of the structured members — `{ a: 2 }` against `undefined` — and the
  leftover-to-naked-parameter step after it was dead code for that case. The
  guard is gone; the leftover is handed to the naked type parameter, which is
  what tsc does with an unmatched source against a union target holding one.
- **Fresh object literals did not widen.** `widens_a_fresh_literal_argument`
  only knew primitive literal *expressions*, so `subject({ n: 1 })` bound `T` to
  `{ n: 1 }` where tsc widens to `{ n: number }`. Object and array literals now
  count as fresh; a `const` assertion still keeps its literals; a constrained
  `T` still keeps them too (the existing rule, unchanged).

## query-core: MutationObserver constructor inference (2026-09-13)

tanstack-query **15 → 13**, every other corpus byte-identical. Four false
positives sat on `new MutationObserver(client, options)`; two are closed and the
other two are blocked one layer down.

**Closed — `observer.mutate(1)` against `void` (2).** The options carry
`onSuccess: vi.fn()`, and tsc infers `TVariables = any` from it: the mock's call
signature is `(...args: any[]) => any`, and a rest parameter's *element* lines
up with every expected parameter from its position on. surge's callback arm
zipped positionally — the first expected parameter got `any[]`, the rest got
nothing — and only accepted a `Type::Function`, which a `Mock<…>` interface is
not. Both fixed in `collect_inferred_type_argument`; the bare-`any` skip that
protects an un-annotated arrow parameter stays, since a rest element is written,
never sketched. Pinned by `callback-rest-any-inference-basic` and two tests in
`generic_literal_widening.rs`.

**Also landed — inferred class type arguments are written back concretely.**
`generic_class_instance_type` used to synthesize `Named(param)` arguments and
resolve them through the inference substitution. That binds the argument, but a
member's lazy alias reference (`subscribe(listener: Listener<A, B, C>)`)
re-reads its parsed arguments later, past the substitution, and came out with
the class's own parameters unbound — reduced with a two-module `Obs extends
Subscribable<Listener<A, B, C>>`. Inferred arguments the syntax can spell
(primitives, literals, arrays, tuples, unions of those) are now reified into
parsed types, so the instance is built exactly as an explicit `new C<string,
…>()` is; anything nominal keeps the name route. Corpora byte-identical.

**Blocked — `mutation.subscribe((state) => states.push(state))` (2).** The
listener's `TData` must come from `mutationFn: (text: string) =>
sleep(10).then(() => text)`, and the value of a *generic method call* on an
interface receiver is the degradation sentinel in this program: probed in the
corpus itself, `Promise.resolve(1)`, `sleep(10).then(() => 'x')` and a user
`Box<string>.map(v => v.length)` all assign to `string` without a diagnostic,
while `[1, 2].map(...)` (the array special case) reports. Property calls do not
instantiate a method's own type parameters, so a method whose parameter has no
default (`map<U>`, `resolve<T>`, `then<TResult1>`) yields `unknown`. That is a
false-negative class of its own, well beyond these two lines; the
`trpc-fn-80` branch carries a written-signature attachment for generic methods
that is the likely repair, and this pair should be re-measured after it lands.

## trpc-fn-80 merge (2026-09-13)

`trpc-fn-80` (trpc `tsc`-only 116 → 89, surge-only 0) merged into main at
`8d22087`, after the overload-selection and tanstack-query commits. The branch
was one commit behind main's new work and both touched
`checks/call/instantiate.rs`; the two conflicts were the explicit-type-argument
seeding order (the branch's earlier seeding kept) and the union candidate guard
(the branch's `matches.is_empty() && naked.is_empty()` kept — equivalent to
main's removal whenever a naked member exists).

Gates on the merged tree: sweep 212/212; trpc 1244 / 1155, surge-only 0,
`tsc`-only 89; ky, ofetch, ts-pattern, unnamed byte-identical; tanstack-query
13 (one `TS7030` closed by the branch's implicit-return work, one opened:
`mutation.test.tsx:1250`, a `vi.fn(impl)` mock whose call signature is not
modelled, recorded on the branch); zod 21 → 25.

Of zod's four, two are the branch's own — `z.ZodError.assert(err)` no longer
narrows a `useUnknownInCatchVariables` catch variable because `ZodError` is a
generic class whose value side is `any`, and modelling generic-class statics was
measured on the branch at +19 (`TS2351`/`TS2554` across zod, trpc, ofetch) and
rejected. The other two were an **interaction**: with written signatures now
attached to generic methods, `registry.get(schema)?.id` reached overload return
selection through a fold whose members were never instantiated, and selection
handed back the declaration's `$replace<Meta, S>` with `Meta` and `S` unbound.
Bisected by toggling each mechanism on the merged tree (selection off → 23,
candidate union off → 25). Selection now keeps the fold's return whenever the
picked member's return still names a type parameter or the sentinel; zod is
23 with every other corpus byte-identical, verified in a clean worktree because
the main checkout was being edited by another session at the time.

Cleanup with the merge: worktrees `surge-ts-fn80` and a stale scratch worktree
removed; merged branches deleted; `recovery/overload-only` kept as the tag
`archive/recovery-overload-only` (7cd5b23) for its `TS2769` and interface-member
group work, which main still lacks.

## tanstack-query after the merge: three more (2026-09-13)

tanstack-query **13 → 10**, every other corpus byte-identical, sweep 214/214.
Worked on a detached worktree (`surge-ts-verify`, branch `fp-burndown`) because
the main checkout was being edited by another session throughout.

- **A package re-export's `Array<string>` indexed as a missing property (2).**
  `const key = queryKey()` where `queryKey` is a `const` arrow with an
  `Array<string>` return annotation reaching the test through
  `@tanstack/query-test-utils` (a pnpm-linked workspace package, `exports` →
  `src/index.ts` → `export { queryKey } from './queryKey'`). The return arrives
  as a nominal reference to the library `Array` interface, where the same
  annotation written locally lowers to `T[]`; `key.length` resolved through the
  interface members, but `key[0]` peeled to a member object with no numeric
  index signature and the literal-key arm reported the *receiver* as the absent
  property (`Property 'key' does not exist on type 'Array<string>'`). The
  index-access dispatch now treats a one-argument `Array` / `ReadonlyArray`
  reference as `T[]`, the same recognition the `Array.isArray` guard uses.
  Reproduced only with the arrow-const + package-entry shape; a `declare
  function` or a relative import never produced the reference.
- **`!!x` as an `&&` operand proved nothing (1).** `streamedQuery`'s `const
  isRefetch = !!query && query.isFetched(); if (isRefetch && refetchMode ===
  'reset') { query.setState(…) }`. The alias expansion was already in place
  (`resolved_alias_condition`), and `!!query` alone narrowed; the reference-guard
  walk over `&&` operands had no case for a double negation, so neither the
  aliased nor the inline form narrowed `query`. Unwrapped there. A single `!`
  still proves the opposite, pinned as the preset's control.
- **Also landed, corpus-neutral:** the declared-union narrowing by a plain
  initializer (`let client: PersistedClient | undefined = persistedClient`) no
  longer refuses a nominal reference because a member deep inside it resolved to
  the sentinel; probed in the persister packages, it narrows in plain functions
  now, but the aggregate's two persister hits sit inside
  `asyncThrottle(async (persistedClient) => …)` as a `Persister` property, where
  the callback parameter is typed only if `TArgs` is inferred back from the
  contextual *return* type of an un-annotated generic — surge matches the
  written return annotation there and `asyncThrottle` has none. Parked.

Remaining 10: eslint-plugin-query 5 (typescript-eslint augmentation through
heritage), MutationObserver `subscribe` 2 (blocked on generic-method call
results being the sentinel), persister 2 (above), `vi.fn(impl)` `Mock<T>` 1
(the merge's known residue).
