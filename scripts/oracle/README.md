# TypeScript Oracle Comparison

This workspace keeps a pinned root `typescript` dev dependency and a small
comparison harness for measuring `typescript-rust` against the TypeScript
compiler.

The Node toolchain is dev-only. Rust crates do not depend on Node tooling, and
`cargo test` does not require `pnpm install`.

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

Compare a disposable local project:

```bash
pnpm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
```

Run the parser and comparison tests:

```bash
pnpm run oracle:test
```

## What it does

- Runs `pnpm exec tsc --noEmit --pretty false --project <tsconfig>`.
- Runs `cargo run -q --manifest-path Cargo.toml -p typescript-rust-cli -- --project <tsconfig> --format json`.
- Normalizes both diagnostic streams to code, file name, line, and column when
  available.
- Compares code counts first, then `(fileName, code)` counts, then
  `(fileName, code, line)` where both sides have line data.

## What it does not do

- It does not require exact message parity.
- It does not require exact span parity.
- It does not add package resolution, `node_modules` lookup, `paths` /
  `baseUrl`, `lib.d.ts`, declaration files, project references, or any broader
  TypeScript parity work.
- It is a measurement tool, not a claim that the checker fully matches
  TypeScript.

## Output levels

- Level 1: by code
- Level 2: by file and code
- Level 3: by file, code, and line when available

The default comparison prints all three levels when possible. Mismatches are
informational unless you pass `--failOnMismatch` or `--strictCodes`.
