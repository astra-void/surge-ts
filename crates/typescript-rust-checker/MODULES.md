# Modules

v0.61 expands the existing relative module-resolution-lite boundary with a
small, pinned module syntax surface for loaded `.ts` files. v0.65 hardens the
ambient-module side of that surface, v0.67 matches TypeScript's TS2882 priority
for unresolved side-effect imports, and v0.84 hardens source/declaration export
visibility for already-loaded modules without adding full package resolution,
`node_modules` runtime semantics, or full TypeScript parity.

## What Is Supported

All relative module-syntax forms are limited to already loaded relative `.ts` files.

- Default imports: `import DefaultThing from "./user";`
- Namespace imports: `import * as user from "./user";`
- Default exports: `export default function ...`, `export default "Ada"`, `export default 123`, `export default true`, and small expression forms the parser already models
- Named re-exports: `export { User } from "./user";`
- Default re-exports: `export { default as DefaultThing } from "./user";`
- Type-only named re-exports: `export type { User } from "./user";`
- Mixed named re-exports: `export { type User, value as renamedValue } from "./user";`
- Star re-exports: `export * from "./user";`
- Namespace re-exports: `export * as userNs from "./user";`
- Named export lists and wrapped declarations from the earlier module-resolution-lite phase
- Side-effect imports and `export {}` module markers

Namespace imports bind a single value symbol whose object type is built from the source module's visible value exports. Default imports bind only a value symbol. Type-only exports stay in the type namespace. Star re-exports forward named value exports and named type exports, but they do not forward default exports. Namespace re-exports bind a conservative namespace object from the target module's visible exports.

## Current Policy

- Script files still share top-level `type` aliases, `interface` declarations, and function declarations across files.
- Module files keep their own top-level declarations local to the file.
- Module files do not contribute to the global script namespace.
- Module files do not see script globals under the isolated-module policy.
- Relative resolution only covers already loaded `./`, `../`, `.` and `..` specifiers.
- Missing ordinary relative modules emit TS2307.
- Missing side-effect imports emit TS2882, matching TypeScript's priority for
  `import "pkg";` / `import "./missing";`.
- Missing exported members emit TS2305.
- Unsupported module syntax stays parser-safe and is pinned with `typescript-rust::unsupported-module-syntax`.
- `export * from` follows a pinned conflict policy: local explicit exports win, and the first star export wins when multiple star exports provide the same name.
- Unresolved star re-exports are intentionally kept from cascading extra consumer diagnostics.
- Bare directory specifiers like `.` and `..` can resolve to already loaded `index.*` files in the same directory graph.
- Relative resolution checks the already-loaded graph in this order: exact target, `.ts`, `.tsx`, `.d.ts`, `.mts`, `.cts`, `.d.mts`, `.d.cts`, then the `index.*` variants in the same order.
- Explicit `.js` / `.jsx` substitution stays narrow and only uses the oracle-proven source/declaration substitutions for already-loaded files.

## Non-relative imports and package stubs

v0.70 adds support for package declaration subpath entrypoints. It resolves exact subpath imports (e.g. `pkg/subpath` or `@scope/pkg/subpath`) and exact `exports["."].types` / `exports["./x"].types` declaration entrypoints. Resolved package files act as external modules.

Default mode for unresolved packages:
- reports TS2307 for ordinary non-relative module specifiers
- reports TS2882 for non-relative side-effect imports
- inserts unknown type/value stubs where possible

`--stubExternalModules`:
- suppresses non-relative missing-module diagnostics, including TS2307 and the
  side-effect-import TS2882 form
- keeps unknown stubs
- leaves relative missing modules and resolved package declaration errors unchanged

## Still Unsupported

These forms remain intentionally out of scope for v0.70.1:

- full package resolution remains unsupported
- only exact `exports.types` declaration targets are supported; full exports maps are not
- exact package declaration subpaths are supported; wildcard/runtime subpaths are not
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- `lib.d.ts` loading and `@types` discovery remain unsupported. v0.72/v0.72.1 uses synthetic built-ins (e.g. `console`, `Promise`, `Math`), not physical `lib.d.ts`. `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics, and v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` using concrete object/interface shapes plus string-literal key unions. That lowering is still not the full utility-type ecosystem: `Required`, `Readonly`, `ReturnType`, `Parameters`, `Awaited`, and conditional-type-backed utilities remain out of scope, and `Record<string, T>` / index-signature style behavior remains unsupported. `noLib: true` disables these synthetic built-ins. DOM, Node, `@types`, and true lib loading remain unsupported.
- The v0.64/v0.65 declaration-ingestion foundation supports a small loaded `.d.ts` ambient subset, including exact `declare module "pkg"` blocks.
  - Ambient modules and resolved package entrypoints resolve before package stubbing.
  - Default import, namespace import, named re-export, type-only re-export, and star re-export behavior inside exact ambient modules is pinned.
  - Duplicate ambient module declarations are first-wins / pinned, not full merging.
  - Exact specifier only.
  - No wildcard ambient module support.
- `import = require(...)`
- `export =`
- Mixed default + named imports
- Default class exports
- CommonJS semantics

## Notes

- Exported generic aliases and interfaces still use the relative module-resolution-lite pass, with explicit type arguments substituted when the imported declaration is instantiated and trailing defaults applied when callers omit type arguments.
- Constraints remain parser-only metadata in this phase.
- Private helper types stay visible through the current module-resolution-lite pass so imported declarations can still resolve them.
- The next phase should continue from compatibility-report output. Likely follow-ups are `@types` / `lib.d.ts` foundational support.

## Ambient Modules
Imports try to resolve from ambient external modules defined by loaded `.d.ts` files with `declare module "pkg"` before falling back to package stubbing. Unsupported declaration syntax remains parser-safe and emits the pinned unsupported-declaration diagnostic.
