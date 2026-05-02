# TypeScript Oracle Comparison

This workspace keeps a pinned root `typescript` dev dependency and a small
comparison harness for measuring `typescript-rust` against the TypeScript
compiler.

## Install

```bash
npm install
```

The repo uses `package-lock.json` for reproducible dev-tool installs. `node_modules/`
stays untracked.

## Run

Compare a committed fixture:

```bash
npm run oracle:compare -- --project tests/compat-projects/generics-basic/tsconfig.json
```

Compare one of the built-in presets:

```bash
npm run oracle:compare -- --project generics-basic
```

Compare a disposable local project:

```bash
npm run oracle:compare -- --project .local-projects/<project>/tsconfig.json --maxDiagnostics 200
```

## What it does

- Runs `tsc --noEmit --pretty false --project <tsconfig>`.
- Runs `cargo run -p typescript-rust-cli -- --project <tsconfig> --format json`.
- Normalizes both diagnostic streams to code, file name, line, and column when
  available.
- Compares code counts first, then `(fileName, code)` counts, then
  `(fileName, code, line)` where both sides have line data.

## What it does not do

- It does not require exact message parity.
- It does not require exact span parity in the first version.
- It does not add package resolution, `node_modules` lookup, `paths` /
  `baseUrl`, `lib.d.ts`, declaration files, project references, or any broader
  TypeScript parity work.
- It is a measurement tool, not a promise that the checker fully matches
  TypeScript.

## Output levels

- Level 1: by code
- Level 2: by file and code
- Level 3: by file, code, and line when available

The default comparison prints Level 1 and Level 2 results and also reports the
Level 3 breakdown when line data exists. Mismatches are informational unless you
pass `--failOnMismatch` or `--strictCodes`.
