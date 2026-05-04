# Declaration Ingestion Foundation

v0.65 hardens the v0.64 declaration ingestion foundation. v0.69 supports narrow bare package declaration entrypoints, and v0.69.1 hardens this support. v0.70 supports exact package declaration subpaths and `exports["types"]`.

## Capabilities
- Loads `.d.ts` files from tsconfig `files` and `include`.
- Parses a narrow ambient declaration subset from loaded declaration files.
- Registers global `declare type`, `declare interface`, `declare const/let/var`, and `declare function` into a shared ambient global namespace.
- Supports exact `declare module "pkg"` blocks for ambient external modules.
- Resolves package `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` fallback.
- Resolves exact package declaration subpaths.
- Checks ambient external modules and resolved package entrypoints before package import stubbing fallback is invoked.
- Supports default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports within the exact ambient-module subset.
- Duplicate ambient module declarations and duplicate ambient globals are first-wins / pinned, not full declaration merging.

## Limitations
- No automatic `lib.d.ts` or `@types` discovery.
- Only exact `exports.types` declaration targets are supported; full exports maps are not.
- Exact package declaration subpaths are supported; wildcard/runtime subpaths are not.
- Full package resolution remains unsupported.
- Unsupported syntax such as `declare class`, `declare namespace`, and other unsupported declaration forms stays pinned with `typescript-rust::unsupported-declaration`.
- No module augmentation, declaration merging, or `import = require()`.
