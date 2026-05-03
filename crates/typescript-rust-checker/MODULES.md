# Modules

v0.61 expands the existing relative module-resolution-lite boundary with a
small, pinned module syntax surface for loaded `.ts` files. v0.65 hardens the
ambient-module side of that surface, and v0.67 matches TypeScript's TS2882
priority for unresolved side-effect imports, without adding package resolution,
`node_modules`, `paths`/`baseUrl`, full declaration-file semantics, CommonJS,
or full TypeScript parity.

## What Is Supported

All relative module-syntax forms are limited to already loaded relative `.ts` files.

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
- Missing ordinary relative modules emit TS2307.
- Missing side-effect imports emit TS2882, matching TypeScript's priority for
  `import "pkg";` / `import "./missing";`.
- Missing exported members emit TS2305.
- Unsupported module syntax stays parser-safe and is pinned with `typescript-rust::unsupported-module-syntax`.
- `export * from` follows a pinned conflict policy: local explicit exports win, and the first star export wins when multiple star exports provide the same name.
- Unresolved star re-exports are intentionally kept from cascading extra consumer diagnostics.

## Non-relative imports and package stubs

v0.63 does not resolve packages. It does, however, stub non-relative imports
and re-exports to reduce compatibility-report cascades.

Default mode:
- reports TS2307 for ordinary non-relative module specifiers
- reports TS2882 for non-relative side-effect imports
- inserts unknown type/value stubs where possible

`--stubExternalModules`:
- suppresses non-relative missing-module diagnostics, including TS2307 and the
  side-effect-import TS2882 form
- keeps unknown stubs
- leaves relative missing modules unchanged

## Still Unsupported

These forms remain intentionally out of scope for v0.61:

- `node_modules` lookup
- `paths` / `baseUrl` resolution
- Full declaration-file semantics and `lib.d.ts` loading remain unsupported.
- The v0.64/v0.65 declaration-ingestion foundation supports a small loaded `.d.ts` ambient subset, including exact `declare module "pkg"` blocks.
  - Ambient modules resolve before package stubbing.
  - Default import, namespace import, named re-export, type-only re-export, and star re-export behavior inside exact ambient modules is pinned.
  - Duplicate ambient module declarations are first-wins / pinned, not full merging.
  - Exact specifier only.
  - No wildcard ambient module support.
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
- The next phase should still be chosen from compatibility-report output rather than by expanding into package or tsconfig-path semantics by default. Likely follow-ups are diagnostic expansion or a package declaration entrypoint foundation.

## Ambient Modules
Imports try to resolve from ambient external modules defined by loaded `.d.ts` files with `declare module "pkg"` before falling back to package stubbing. Unsupported declaration syntax remains parser-safe and emits the pinned unsupported-declaration diagnostic.
