# Physical `lib.d.ts` Loading

Physical TypeScript `lib.d.ts` loading is supported for ES/DOM libs with
parser-safe ingestion, reference-lib graph loading, merged global interfaces,
common index signatures, and meaningful ES/DOM ambient globals. Full
byte-for-byte TypeScript lib semantics remain in progress.

This path is **opt-in**. The generated default-lib subset (see `default_lib`)
stays the default so existing behaviour — including exact auth-kit diagnostics —
is unchanged.

## Enabling

Physical loading turns on when any of these is true:

- the CLI flag `--physicalLibs` is passed (project mode),
- a `.physicalLibs` marker file sits beside the resolved `tsconfig.json`
  (used by the oracle fixtures, which run the CLI with only `--project`), or
- the `TYPESCRIPT_RUST_PHYSICAL_LIBS` environment variable is set.

The resolver walks up from the project root looking for
`node_modules/typescript/lib`. If the package is not installed it warns and
falls back to the generated subset, so `cargo test` never requires
`pnpm install`. Physical-lib fixtures and tests skip when the package is absent.

## What is loaded

1. **Discovery** — the installed `typescript` package's `lib/` directory.
2. **`compilerOptions.lib` mapping** — names like `es2022`, `dom`,
   `dom.iterable` map to `lib.<name>.d.ts`, normalizing case and `lib.`/`.d.ts`
   affixes. When `lib` is unset, the target's `lib.<target>.full.d.ts`
   aggregate seeds the graph, matching how `tsc` derives the default lib.
3. **Reference graph** — `/// <reference lib="..." />` directives are followed
   recursively, deduped by canonical path, ordered dependency-first, and
   cycle-guarded. The scanner skips the leading `/*! license */` banner and
   stops at the first real declaration. `no-default-lib` is ignored (the graph
   is already explicit).

Loaded files are tagged `FileKind::PhysicalDefaultLib`, parsed, and lowered
through the normal ambient-global pipeline. Diagnostics originating inside lib
files are suppressed so unsupported lib syntax cannot flood user diagnostics.

## Supported declaration/type surface

- **Interface declaration merging** for default libs (e.g. `PromiseConstructor`,
  `Array<T>`, DOM `Window` split across many files): members and `extends`
  clauses are concatenated rather than first-wins.
- **`declare var` / `declare function` globals** registered as ambient values
  (`Math`, `JSON`, `Date`, `Promise`, `Symbol`, `console`, `fetch`, …).
- **`new X()` instance resolution** to the real `X` interface instance
  (`Map<K, V>`, `Date`, `URL`, `Response`, `EventTarget`, …), preferring the
  loaded interface over the hardcoded builtin fast-path.
- **Generic interface methods/properties** (`Map.get` -> `V | undefined`,
  `Date.getTime()` -> `number`, `URL.pathname` -> `string`, `Response.ok` ->
  `boolean`).
- **String/number index signatures** (`interface Env { [key: string]: string |
  undefined }`), including inheritance from a base interface; property and
  bracket access fall back to the index type.
- **`readonly` members** are parsed as ordinary properties (readonly-ness is not
  enforced, but the property is present).
- **`this`-returning methods** (e.g. `Map.set`) parse as `any`-returning so the
  member is not dropped.
- **`Promise<T>` / `PromiseLike<T>`** are modelled as their resolved value `T`
  (an implicit await everywhere), since `await` is stripped at parse time. This
  lets async/await code typecheck against the resolved type.
- **`noLib: true`** disables physical (and generated) default libs.
- **Configured `@types`** compose with physical libs without duplicate-global
  explosion.

## Known gaps (in progress)

- **Overload resolution** — only one signature is used per symbol, so valid
  calls against overloaded lib APIs can produce spurious `TS2554` arity errors.
  This is the dominant source of the physical-mode auth-kit delta.
- **`Awaited<T>`** and `Promise.resolve`/`Promise.all` precise typing — the
  `Promise<T>` -> `T` collapse covers `await`, but utility-conditional awaited
  inference is not modelled, so some awaited values resolve to `unknown`.
- **Call/construct signatures on interfaces** — anonymous `(...)` / `new (...)`
  members are dropped, so e.g. `Symbol("x")` reports `TS2349` (not callable).
- **Contextual callback parameter typing** — `addEventListener("click", e =>
  …)` leaves `e` implicitly `any` (`TS7006`) instead of `Event`.
- **`.then()`-style promise chaining** on a raw promise (the collapse removes
  the structural `Promise` surface).
- Full DOM coverage beyond everyday types is not guaranteed.

## Oracle fixtures

`tests/compat-projects/physical-lib-*` are oracle-compared against `tsc`
(TypeScript 6.0.3). At time of writing, full file/code/line parity holds for
`es-array`, `es-map-set`, `dom-url`, `index-signature`, `with-configured-types`,
and `no-lib`. `es-promise` and `dom-fetch` under-report (missing error, no false
positive). `dom-event` (contextual callback) and `reference-graph` (Symbol call
signature) diverge per the known gaps above.
