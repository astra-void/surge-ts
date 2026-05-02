# Modules

v0.61 expands the existing relative module-resolution-lite boundary with a
small, pinned module syntax surface for loaded `.ts` files. It keeps package
resolution, `node_modules`, `paths`/`baseUrl`, declaration files, CommonJS,
and full TypeScript parity out of scope.

## What Is Supported

All supported forms are limited to already loaded relative `.ts` files.

- Default imports: `import DefaultThing from "./user";`
- Namespace imports: `import * as user from "./user";`
- Default exports: `export default function ...`, `export default "Ada"`, `export default 123`, `export default true`, and small expression forms the parser already models
- Named re-exports: `export { User } from "./user";`
- Type-only named re-exports: `export type { User } from "./user";`
- Star re-exports: `export * from "./user";`
- Named export lists and wrapped declarations from the earlier module-resolution-lite phase
- Side-effect imports and `export {}` module markers

Namespace imports bind a single value symbol whose object type is built from the source module's value exports. Default imports bind only a value symbol. Type-only exports stay in the type namespace. Star re-exports forward named value exports and named type exports, but they do not forward default exports.

## Current Policy

- Script files still share top-level `type` aliases, `interface` declarations, and function declarations across files.
- Module files keep their own top-level declarations local to the file.
- Module files do not contribute to the global script namespace.
- Module files do not see script globals under the isolated-module policy.
- Relative resolution only covers already loaded `./` and `../` specifiers.
- Missing relative modules emit TS2307.
- Missing exported members emit TS2305.
- Unsupported module syntax stays parser-safe and is pinned with `typescript-rust::unsupported-module-syntax`.
- `export * from` follows a pinned conflict policy: local explicit exports win, and the first star export wins when multiple star exports provide the same name.
- Unresolved star re-exports are intentionally kept from cascading extra consumer diagnostics.

## Still Unsupported

These forms remain intentionally out of scope for v0.61:

- Non-relative package imports and re-exports
- `node_modules` lookup
- `paths` / `baseUrl` resolution
- Declaration files and `lib.d.ts`
- `import = require(...)`
- `export =`
- `export * as Foo from "./foo"`
- Mixed default + named imports
- Default class exports
- CommonJS semantics

## Notes

- Exported generic aliases and interfaces still use the relative module-resolution-lite pass, with explicit type arguments substituted when the imported declaration is instantiated and trailing defaults applied when callers omit type arguments.
- Constraints remain parser-only metadata in this phase.
- Private helper types stay visible through the current module-resolution-lite pass so imported declarations can still resolve them.
- The next phase should still be chosen from compatibility-report output rather than by expanding into package or tsconfig-path semantics by default.
