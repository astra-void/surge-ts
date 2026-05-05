# Real Project Compatibility

`v0.60.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass. `v0.60` adds a TypeScript
oracle comparison harness on top of that baseline so we can measure the current
checker against a pinned compiler without changing the checker to chase parity.

v0.68.1 hardens the diagnostic coverage metadata, ensuring that `support = "emitted"` accurately reflects current checker capabilities and is backed by testing.

v0.70 supports package declaration subpath entrypoints.
v0.69 supports narrow bare package declaration entrypoints.
v0.69.1 hardens/refactors this support. v0.72/v0.72.1 uses synthetic built-ins, not physical `lib.d.ts`. `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. Utility types mostly suppress TS2304 and do not implement mapped/conditional type semantics yet. `noLib: true` disables synthetic built-ins. DOM, Node, `@types`, and true lib loading remain unsupported.
Supported: types, typings, index.d.ts, bare scoped/unscoped packages, exact declaration subpaths, exports["types"] condition.
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

## Note on Optional Chaining and Nullish Coalescing (v0.74)
v0.74 supports a narrow optional chaining and nullish coalescing subset. Optional property access and optional calls return `T | undefined` under the current conservative policy. Nullish coalescing (`??`) removes `undefined` from the left side in the supported subset. Full control-flow narrowing, optional element access, deeply nested chains, `??=`, and `null`-accurate semantics remain unsupported.

## Note on Benchmark Harness (v0.75)
v0.75 adds a compiler speed benchmark harness (`scripts/bench/compare-compilers.ts`) along with diagnostic-drift-aware reporting. This is a developer-facing regression tool comparing no-emit project checks across `tsc`, `tsgo` (optional), and the `typescript-rust-cli` release binary. It enforces a TS 7-oriented policy that avoids `ignoreDeprecations` in committed fixtures and requires looking at semantic equivalence alongside wall-clock performance.
