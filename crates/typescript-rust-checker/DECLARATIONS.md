# Declaration Ingestion Foundation

v0.65 hardens the v0.64 declaration ingestion foundation. v0.69 supports narrow bare package declaration entrypoints, v0.69.1 hardens this support, v0.70 supports exact package declaration subpaths and `exports["."].types`, v0.84 hardens declaration-file export visibility for already-loaded modules without adding full declaration semantics, and v0.85 adds a generated default-lib foundation that loads a small supported subset from the local TypeScript package as ambient declarations.

## Capabilities
- Loads `.d.ts` files from tsconfig `files` and `include`.
- Parses a narrow ambient declaration subset from loaded declaration files.
- Registers global `declare type`, `declare interface`, `declare const/let/var`, and `declare function` into a shared ambient global namespace.
- Supports exact `declare module "pkg"` blocks for ambient external modules.
- Resolves package `.d.ts` entrypoints via `types`, `typings`, exact `exports["."].types` / `exports["./x"].types`, or `index.d.ts` fallback.
- Resolves exact package declaration subpaths.
- Preserves export visibility through loaded declaration files for `export { ... } from ...`, `export type { ... } from ...`, `export * from ...`, and `export * as ns from ...` when the target declaration file is already loaded or resolved.
- Checks ambient external modules and resolved package entrypoints before package import stubbing fallback is invoked.
- Supports default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports within the exact ambient-module subset.
- Duplicate ambient module declarations and duplicate ambient globals are first-wins / pinned, not full declaration merging.
- Declaration files remain symbol sources even when `skipLibCheck` suppresses their diagnostics in dependency graphs.

## Limitations
- No automatic `lib.d.ts` or `@types` discovery. The generated default-lib subset is loaded separately and is not the same as full upstream `lib.d.ts` parity.
- Only exact `exports["."].types` / `exports["./x"].types` declaration targets are supported; full exports maps are not.
- Exact package declaration subpaths are supported; wildcard/runtime subpaths are not.
- Full package resolution remains unsupported.
- No runtime JS entrypoint resolution, `main`, `import` / `require` conditions, or wildcard/pattern exports.
- Unsupported syntax such as `declare class`, `declare namespace`, and other unsupported declaration forms stays pinned with `typescript-rust::unsupported-declaration`.
- No module augmentation, declaration merging, or `import = require()`.
