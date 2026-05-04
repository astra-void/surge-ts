# Real Project Compatibility

`v0.60.1` is still an instrumentation baseline for real-project compatibility,
not a claim that large TypeScript packages pass. `v0.60` adds a TypeScript
oracle comparison harness on top of that baseline so we can measure the current
checker against a pinned compiler without changing the checker to chase parity.

v0.68.1 hardens the diagnostic coverage metadata, ensuring that `support = "emitted"` accurately reflects current checker capabilities and is backed by testing.

v0.69 adds narrow package declaration entrypoint support for bare package imports in project mode.
It resolves package `.d.ts` files via `types`, `typings`, or `index.d.ts` fallback without full package resolution.

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

- package resolution
- `node_modules` lookup
- `paths` / `baseUrl`
- lib.d.ts modeling or auto-loading
- full declaration-file semantics
- `@types` discovery
- package `exports` / `main` / JS runtime entrypoints
- package subpath imports (e.g. `pkg/subpath`)
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
- bare package imports (e.g. `pkg` or `@scope/pkg`) resolve to declaration entrypoints (`types`, `typings`, or `index.d.ts` fallback) in project mode
- resolved package `.d.ts` files act as external modules and do not leak private helpers globally
- default import, namespace import, and re-export behavior for ambient modules and package entrypoints is pinned
- duplicate ambient module and duplicate ambient global behavior is pinned, not merged
- unsupported declaration syntax remains parser-safe and emits stable diagnostics
- TS2882 is catalog-backed and is emitted for unresolved side-effect imports such as `import "reflect-metadata";`
- ordinary missing package imports still produce TS2307 by default
- `--stubExternalModules` suppresses non-relative missing-module diagnostics, including the side-effect TS2882 form, while leaving relative missing modules and resolved package declaration errors unchanged
- full package resolution, `exports` maps, package subpath imports, `@types`, and lib.d.ts discovery are still out of scope

The oracle harness also stays away from those areas. It only measures the
current surface against TypeScript diagnostics; it does not add new resolver or
type-system behavior to make the numbers line up.
File mode is intentionally narrow: it only accepts `.ts` source files for now,
and it is a quick standalone oracle rather than the main compatibility path.

The next phase should still be chosen from oracle and compat-report output, not
from a fixed feature wish list. Module syntax expansion, package import
stubbing, declaration-file ingestion, ambient declaration hardening, and the
diagnostic catalog/codegen foundation are implemented. Likely follow-ups are
diagnostic expansion or package subpath declaration resolution / exports types condition.
