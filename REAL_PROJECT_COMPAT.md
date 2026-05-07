# Real Project Compatibility

`v0.60.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass. `v0.60` adds a TypeScript
oracle comparison harness on top of that baseline so we can measure the current
checker against a pinned compiler without changing the checker to chase parity.

v0.82 is a project visibility and file-discovery hardening phase. It does not
claim full real-project parity. The goal is to make silent zero-file project
comparisons impossible, especially when `tsc` sees `.tsx`, `.mts`, `.cts`,
`.d.ts`, and nested `examples/**` inputs that the Rust loader might otherwise
miss. `.tsx` visibility is not the same as JSX or React type support.

## v0.84 Real-Project Audit

The old `trpc` baseline is retired as the active real-project target. `auth-kit` is
the intended finite baseline for this phase, but this workspace does not currently
contain `.local-projects/auth-kit`, so the measured metrics below are pending in
this checkout rather than fabricated from another project.

Preflight commands to run once the `auth-kit` checkout is available:

- `cargo fmt --check`
- `cargo test`
- `pnpm run oracle:test`
- `pnpm run bench:test`
- `cargo run -q -p typescript-rust-cli -- --project .local-projects/auth-kit/tsconfig.json --showConfig`
- `cargo run -q -p typescript-rust-cli -- --project .local-projects/auth-kit/tsconfig.json --compatReport --maxDiagnostics 200`
- `pnpm run oracle:compare -- --project .local-projects/auth-kit/tsconfig.json --maxDiagnostics 200`

Measured real-project state for `.local-projects/auth-kit/tsconfig.json`:

| Metric | Value |
| --- | ---: |
| TypeScript diagnostics | pending |
| typescript-rust diagnostics, raw oracle compare | pending |
| typescript-rust diagnostics, compat-report JSON | pending |
| loaded files total | pending |
| root source files | pending |
| root declarations | pending |
| dependency declarations | pending |
| generated files | pending |
| dependency JavaScript source files loaded | pending |
| diagnostics from dependency declarations | pending |
| diagnostics from dependency JavaScript source files | pending |
| Rust-only `typescript-rust::*` diagnostics in `tsc` profile | pending |

When the audit is rerun, the report should explicitly note whether raw oracle
compare and compat-report totals differ, and if they do, capture the counting
path that needs to be fixed later instead of hiding the gap.

The current compat-report and oracle compare surfaces already split dependency
declarations from dependency JavaScript source noise, and the oracle compare
classifier now separates missing synthetic built-in candidates from ES/lib-lite
globals and Node/@types globals. That makes the next phase choice clearer once
the auth-kit metrics are available.

The synthetic builtin pack remains intentionally narrow and synthetic: it covers
`Array.from`, `Date.now`, `Number`, `String`, `Boolean`, `Math`, `JSON`,
`Object`, `Map`, `Uint8Array`, `globalThis`, and `isNaN` without pretending to
load a physical `lib.d.ts`.

v0.68.1 hardens the diagnostic coverage metadata, ensuring that `support = "emitted"` accurately reflects current checker capabilities and is backed by testing.

v0.77.1 implements non-null assertions and a parser-safe `as const` foundation under the default `tsc` diagnostic profile. Literal types and tuple constraints are preserved on primitive literals and object/array properties for `as const` expressions. `satisfies` with `as const` behaves correctly. Optional chaining AST evaluation now correctly propagates the `undefined` short-circuit across subsequent non-null assertions (e.g. `a?.b!.c` evaluates to `C | undefined`).
v0.74.1 supports nested optional property/call chains in a conservative way, and optional element access for arrays and tuples. Every optional chain segment still widens the result with `undefined`. `??` removes `undefined` only in the supported subset. `null`-accurate semantics and control-flow narrowing remain unsupported. `ignoreDeprecations` is not used in committed fixtures because TS 7-oriented compatibility should not hide deprecated option behavior.
v0.70 supports package declaration subpath entrypoints.
v0.69 supports narrow bare package declaration entrypoints.
v0.69.1 hardens/refactors this support. v0.72/v0.72.1 uses synthetic built-ins, not physical `lib.d.ts`. `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of the mapped-type foundation introduced in v0.80.1. This is still not full utility-type support: `Required`, `Readonly`, `ReturnType`, `Parameters`, `Awaited`, and conditional-type-backed utilities remain unsupported or synthetic noise reducers, and `Record<string, T>` / index-signature style behavior remains unsupported unless a later phase proves it with oracle evidence. Physical `lib.d.ts`, `@types`, DOM, Node, and true lib loading remain unsupported. `noLib: true` disables synthetic built-ins.
Supported: types, typings, index.d.ts, bare scoped/unscoped packages, exact declaration subpaths, exact `exports["."].types` / `exports["./x"].types` declaration targets.
Unsupported: exports runtime conditions, main, typesVersions, wildcard exports, @types, physical `lib.d.ts` loading, DOM/Node globals, baseUrl resolution, JS runtime entrypoints, rootDirs, project references.

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
cargo run -p typescript-rust-cli -- --project .local-projects/<project>/tsconfig.json --compatReport --maxDiagnostics 200
pnpm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
pnpm run oracle:compare -- --file examples/basic.ts
pnpm run oracle:compare -- --file examples/basic.ts --ignoreConfig
```

## What the report tells you

The compatibility report is a triage tool. It helps separate the first-order
blockers from the noise:

1. Parser errors
2. Unsupported module syntax
3. Non-relative package imports and side-effect import diagnostics
4. Missing global/lib symbols or unsupported generic syntax
5. Plain type mismatches

The report does not guarantee that a project is expected to pass.
The oracle comparison does not guarantee that message text or exact spans
match; it starts with code, file, and line/column normalization first.
Diagnostic codes and messages are catalog-driven in `typescript-rust-diagnostics`,
so catalog updates can legitimately move oracle output even when checker
semantics stay the same.
Use `--project` for `tsconfig.json`-based projects and `--file` for single
source files. Passing a `.ts` file to `--project` is rejected now so TypeScript
does not misread the file as a config input.

## Current baseline

The current baseline still intentionally avoids:

- full package resolution remains unsupported
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- lib.d.ts modeling or auto-loading
- full declaration-file semantics
- `@types` discovery
- only exact `exports.types` declaration targets are supported; full exports maps are not
- exact package declaration subpaths are supported; wildcard/runtime subpaths are not
- project references
- incremental or watch behavior
- generic inference and generic classes
- enums and namespaces
- CommonJS or bundler semantics
- generic constraints enforcement
- generic call-site inference
- mixed default + named imports
- default class exports
- v0.81 only lowers `Record`, `Partial`, `Pick`, and `Omit` in a narrow synthetic path; the rest of the utility-type ecosystem remains out of scope

The current declaration and diagnostic baseline includes:

- exact ambient `declare module "pkg"` blocks are supported
- ambient modules resolve before package stubbing
- bare package imports (e.g. `pkg` or `@scope/pkg`) and exact subpaths resolve to declaration entrypoints (`types`, `typings`, `exports["types"]`, or `index.d.ts` fallback) in project mode
- resolved package `.d.ts` files act as external modules and do not leak private helpers globally
- default import, namespace import, and re-export behavior for ambient modules and package entrypoints is pinned
- duplicate ambient module and duplicate ambient global behavior is pinned, not merged
- unsupported declaration syntax remains parser-safe and emits stable diagnostics
- TS2882 is catalog-backed and is emitted for unresolved side-effect imports such as `import "reflect-metadata";`
- ordinary missing package imports still produce TS2307 by default
- `--stubExternalModules` suppresses non-relative missing-module diagnostics, including the side-effect TS2882 form, while leaving relative missing modules and resolved package declaration errors unchanged
- full package resolution, wildcard `exports`, JS runtime subpaths, `@types`, and lib.d.ts discovery are still out of scope

The oracle harness also stays away from those areas. It only measures the
current surface against TypeScript diagnostics; it does not add new resolver or
type-system behavior to make the numbers line up.
File mode is intentionally narrow: it only accepts `.ts` source files for now,
and it is a quick standalone oracle rather than the main compatibility path.

The next phase should still be chosen from oracle and compat-report output, not
from a fixed feature wish list. Module syntax expansion, package import
stubbing, declaration-file ingestion, ambient declaration hardening, and the
diagnostic catalog/codegen foundation are implemented. Current likely blockers are common expression syntax, ambient `@types`, DOM/Node globals, and true lib semantics.

## Note on Type Assertions (v0.73)
Type assertions (`as` expressions) were chosen for v0.73 because they are extremely common in real TypeScript projects, particularly around parsed data, library boundaries, and compatibility shims. By implementing a narrow parsing and inference surface for primitive assertions, aliases, and built-in arrays, we significantly reduce false-positive TS2322 cascades without needing full TypeScript assertion semantics. Dominant blockers remaining after this phase continue to revolve around ambient `@types` package discovery, missing DOM/Node globals, and `lib.d.ts` semantics which often surface as TS2304 errors.

## Note on Optional Chaining and Nullish Coalescing (v0.74/v0.74.1)
v0.74.1 supports nested optional property/call chains in a conservative way, and optional element access for arrays and tuples. Every optional chain segment still widens the result with `undefined`. `??` removes `undefined` only in the supported subset. `null`-accurate semantics, full control-flow narrowing, `??=`, and non-null assertions remain unsupported.

## Note on Benchmark Harness (v0.75/v0.75.2)
v0.75/v0.75.2 adds a compiler speed benchmark harness (`scripts/bench/compare-compilers.ts`) along with diagnostic-drift-aware reporting. This is a developer-facing regression tool comparing no-emit project checks across `tsc`, `tsgo` (optional), and the `typescript-rust-cli` release binary. It enforces a TS 7-oriented policy that avoids `ignoreDeprecations` in committed fixtures and requires looking at semantic equivalence alongside wall-clock performance. These are local-machine-relative developer aids; SVG/HTML reports are visualization aids, not marketing claims. Diagnostic drift must be read with timing.

## Note on Type Operators (v0.78)
v0.78 implements a parser-safe foundation for `typeof value`, `keyof T`, and the `keyof typeof constObject` pattern, in a narrow type-position subset. The `typeof` type query resolves top-level or in-scope values to their inferred types. `keyof` resolves object and interface types to string literal unions of their properties. If a value or type is unresolved or unsupported, `typescript-rust` defaults to parser-safe conservative emission, outputting `TS2304` or resolving to `Unknown` to match TypeScript's fallback behavior. Advanced types like `typeof import("pkg")`, namespace/class constructor `typeof`, conditional types, template literal types, index signatures, and exact intersection-of-keys semantics for unions remain unsupported.

## Note on Indexed Access Types (v0.79/v0.79.2)
v0.79 implements a parser-safe indexed access type foundation (`T[K]`, `T[keyof T]`). It supports narrow indexed access types including object/interface string-literal property lookup, `T[keyof T]` value unions, and tuple numeric literal indexing. v0.79.2 fixes unresolved-key indexed access diagnostic parity and non-null assertion optional chain parity, ensuring that the default `tsc` profile emits `TS2304` and `TS2538` cascades correctly, and that optional chain `undefined` propagation behaves accurately around non-null assertions and `satisfies` expressions, matching TypeScript's cascading behavior. Advanced usages like conditional types, template literal types, index signatures, and generic indexed access remain unsupported.

## Note on Mapped Types (v0.80.1)
v0.80.1 supports a narrow mapped type foundation.
Supported: `{ [K in keyof T]: T[K] }` and `{ [K in keyof T]?: T[K] }` over concrete object/interface inputs after explicit generic substitution.
Unsupported: key remapping, conditional types, template literal types, index signatures, readonly mapped semantics, modifier arithmetic, generic inference, `@types`, physical `lib.d.ts`, DOM/Node globals.
Utility types are not automatically "full TypeScript utility types" just because mapped types exist. If `Partial`, `Record`, `Pick`, `Omit` remain synthetic aliases/noise reducers, say so clearly.
