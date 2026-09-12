# Vendored TypeScript Standard Library

This script refreshes the checked-in snapshot of TypeScript's `lib.*.d.ts`
declarations that surge embeds in its binary. It copies from the locally
installed TypeScript package and never touches the network.

TypeScript 7.0 (the native compiler) ships the lib `.d.ts` files inside the
platform-specific binary package rather than in `node_modules/typescript/lib`;
the generator resolves the installed platform package automatically.

## Inputs

- `node_modules/@typescript/typescript-<platform>-<arch>/lib/lib.*.d.ts`
- `node_modules/typescript/LICENSE` and `NOTICE.txt`

## Output

- `crates/surge-ts-checker/generated-libs/lib.*.d.ts`
- `crates/surge-ts-checker/generated-libs/LICENSE.txt`, `NOTICE.txt`
- `crates/surge-ts-checker/generated-libs/manifest.json`

## Updating to a new TypeScript version

```bash
pnpm add -D typescript@npm:typescript@<version>
pnpm run lib:generate
pnpm run lib:test
cargo nextest run --workspace
pnpm run oracle:sweep -- --all --maxDiagnostics 200
```

`lib:generate` records the exact source version and a content hash in
`manifest.json`, copies the upstream license and notice files, and rewrites the
output directory from scratch so stale files cannot survive an update. Output is
deterministic: re-running it on the same input produces byte-identical
repository contents.

The generator fails clearly if the TypeScript package, its lib directory, or
either license file is missing.

Nothing here runs during a normal `cargo build`. The build script only reads the
already-checked-in files, so end users never need npm, node, or TypeScript.
