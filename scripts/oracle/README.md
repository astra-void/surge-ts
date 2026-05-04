# TypeScript Oracle Comparison

This workspace keeps a pinned root `typescript` dev dependency and a small
comparison harness for measuring `typescript-rust` against the TypeScript
compiler.

v0.68 uses oracle comparisons to validate emitted diagnostic coverage. The new `diagnostics-pack` fixture is a synthetic example that should stay tiny and reviewable while still exercising real checker emission paths.

The Node toolchain is dev-only. Rust crates do not depend on Node tooling, and
`cargo test` does not require `pnpm install`.

## Known Mismatches

- **Duplicate Declarations (`TS2393`, `TS2451`)**: TypeScript reports diagnostics on *both* the original declaration and the duplicate. `typescript-rust` reports it on only one.
- **Control Flow (`TS2454`)**: TypeScript sometimes emits multiple use-before-assignment diagnostics for the same variable in complex assignments.
- **Generic Arity (`TS2314`, `TS2315`)**: TypeScript reports the error on the *usage* site (e.g. the variable declaration). `typescript-rust` reports it using the span of the *type declaration*.
- **Implicit Any (`TS7005`)**: `typescript-rust` reports this for uninitialized variables globally if `strict` is enabled, while TypeScript has more complex usage-based inference.

## Lockfile policy

- The repository commits `pnpm-lock.yaml`.
- `node_modules/` stays ignored.
- `.local-projects/` stays ignored.
- No yarn lockfile is used.
- The TypeScript version is pinned in the root `package.json`; do not switch it
  to `latest`.

## Install

```bash
pnpm install
```

## Run

Compare a committed fixture:

```bash
pnpm run oracle:compare -- --project tests/compat-projects/generics-basic/tsconfig.json
```

Compare one of the built-in presets:

```bash
pnpm run oracle:compare -- --project generics-basic
```

Compare a single standalone source file using TypeScript-like CLI behavior:

```bash
pnpm run oracle:compare -- --file examples/basic.ts
pnpm run oracle:compare -- --file examples/assignment.ts
```

Compare a single standalone source file while explicitly bypassing TS5112
and running semantic checking:

```bash
pnpm run oracle:compare -- --file examples/basic.ts --ignoreConfig
```

Compare a project while suppressing external package missing-module errors:

```bash
pnpm run oracle:compare -- --project package-imports --stubExternalModules
```
*Note: `--stubExternalModules` is a typescript-rust-only compatibility flag. The oracle does not pass it to TypeScript. In this mode, typescript-rust suppresses non-relative missing-module diagnostics, including TS2307 and the side-effect-import TS2882 form, while TypeScript still reports its normal diagnostics.*

The `package-imports` fixture pins TypeScript 6.0.3 behavior for unresolved
package imports: ordinary imports and re-exports remain TS2307, while a bare
side-effect import such as `import "reflect-metadata";` is TS2882. This is a
diagnostic-priority parity check only; it is not package resolution.

Compare the declaration-ingestion fixture through the preset system:

```bash
pnpm run oracle:compare -- --project declarations-basic
pnpm run oracle:compare -- --project declarations-hardening
```

The declaration presets are pinned compatibility fixtures, not a claim that the oracle handles full package discovery or full declaration merging. Supported: bare packages via types, typings, index.d.ts fallback.


Compare a disposable local project:

```bash
pnpm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
```

Run the parser and comparison tests:

```bash
pnpm run oracle:test
```

## What it does

- Project mode runs:
  ```txt
  pnpm exec tsc --noEmit --pretty false --project <tsconfig>
  cargo run ... -- --project <tsconfig> --format json
  ```
- File mode without `--ignoreConfig` runs:
  ```txt
  pnpm exec tsc --noEmit --pretty false <file>
  cargo run ... -- --format json <file>
  ```
  and may report TS5112 when a `tsconfig.json` is present.
- File mode with `--ignoreConfig` runs:
  ```txt
  pnpm exec tsc --noEmit --pretty false --ignoreConfig <file>
  cargo run ... -- --format json --ignoreConfig <file>
  ```
  and is the explicit standalone semantic comparison mode.
- The oracle never secretly adds `--ignoreConfig`.
- Normalizes both diagnostic streams to code, file name, line, and column when
  available.
- Compares code counts first, then `(fileName, code)` counts, then
  `(fileName, code, line)` where both sides have line data.

## What it does not do

- It does not require exact message parity.
- It does not require exact span parity.
- It does not add full package resolution, `paths` /
  `baseUrl`, full declaration-file semantics, `lib.d.ts`, `@types`,
  or project references. (It only supports declaration-oriented `node_modules` lookup.)
- It does not add declaration merging parity or TypeScript's full ambient-module semantics.
- File mode currently only accepts `.ts` files. Project mode is still the
  preferred oracle for multi-file compatibility checks, and file mode may drift
  from project mode because single-file TypeScript runs use default compiler
  options unless you add explicit flags later.
- It is a measurement tool, not a claim that the checker fully matches
  TypeScript.

## Common Mistake

This is invalid:

```bash
pnpm run oracle:compare -- --project examples/basic.ts
```

`--project` expects a preset name or `tsconfig.json` path. Use `--file` for a
single source file. This prevents TypeScript from treating `.ts` files as
`tsconfig` inputs and emitting misleading config diagnostics.

## Output levels

- Level 1: by code
- Level 2: by file and code
- Level 3: by file, code, and line when available

The default comparison prints all three levels when possible. Mismatches are
informational unless you pass `--failOnMismatch` or `--strictCodes`.
