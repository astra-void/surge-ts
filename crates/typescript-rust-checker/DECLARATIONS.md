# Declaration Ingestion Foundation

v0.65 hardens the v0.64 declaration ingestion foundation so ambient behavior is predictable before any package discovery work lands.

## Capabilities
- Loads `.d.ts` files from tsconfig `files` and `include`.
- Parses a narrow ambient declaration subset from loaded declaration files.
- Registers global `declare type`, `declare interface`, `declare const/let/var`, and `declare function` into a shared ambient global namespace.
- Supports exact `declare module "pkg"` blocks for ambient external modules.
- Resolves package `.d.ts` entrypoints via `types`, `typings`, or `index.d.ts` fallback.
- Checks ambient external modules and resolved package entrypoints before package import stubbing fallback is invoked.
- Supports default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports within the exact ambient-module subset.
- Duplicate ambient module declarations and duplicate ambient globals are first-wins / pinned, not full declaration merging.

## Limitations
- No automatic `lib.d.ts` or `@types` discovery.
- No `exports` map package.json parsing.
- No package subpath imports.
- Full declaration-file semantics remain out of scope.
- Unsupported syntax such as `declare class`, `declare namespace`, and other unsupported declaration forms stays pinned with `typescript-rust::unsupported-declaration`.
- No module augmentation, declaration merging, or `import = require()`.
