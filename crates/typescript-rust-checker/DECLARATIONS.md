# Declaration Ingestion Foundation

v0.64 introduces a minimal declaration ingestion foundation to move toward real TypeScript compatibility.

## Capabilities
- Loads `.d.ts` files according to tsconfig `include`/`files`.
- Parses a limited ambient declaration subset.
- Registers global `declare type`, `declare interface`, `declare const/let/var`, and `declare function` into a shared ambient global namespace.
- Supports `declare module "pkg"` for ambient external modules.
- Ambient modules are checked before package import stubbing fallback is invoked.

## Limitations
- No automatic `lib.d.ts` or `@types` discovery.
- No `node_modules` package.json parsing.
- Unsupported syntax (like `declare class`, `declare namespace`) is pinned with `typescript-rust::unsupported-declaration`.
- No module augmentation, declaration merging, or `import = require()`.
