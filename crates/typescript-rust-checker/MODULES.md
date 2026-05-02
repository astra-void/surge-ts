# Modules

v0.58 keeps the relative module-resolution-lite boundary from v0.57.1 and adds
compatibility-report instrumentation for real-project triage.

## What is parsed

- Named imports: `import { User } from "./user";`
- Type-only imports: `import type { User } from "./user";`
- Side-effect imports: `import "./setup";`
- Exported declarations:
  - `export interface ...`
  - `export type ...`
  - `export function ...`
  - `export const/let/var ...`
- Named export lists:
  - `export { User };`
  - `export { User as UserModel };`
  - `export type { User };`
  - `export type { User as UserModel };`
- Empty export markers: `export {};`

## Module boundary

Any file containing top-level import or export syntax is treated as a module file.

- Script files continue to participate in global-script sharing.
- Module files are isolated for declaration sharing in this phase.
- Relative imports and local named exports participate in a limited program-mode resolution pass over loaded `.ts` files only.
- Named imports bind type and value namespaces separately.
- Side-effect imports resolve a loaded target file and bind nothing.
- Named export lists resolve against same-file local declarations.
- Exported generic aliases and interfaces are preserved across the relative module-resolution-lite pass, explicit type arguments are substituted when the imported declaration is instantiated, and trailing defaults are applied when callers omit type arguments.
- Constraints remain parser-only metadata in this phase.
- Private helper types stay visible through the current module-resolution-lite
  pass, so imported declarations can still resolve them while the model remains
  intentionally narrower than full package/module resolution.
- Unsupported module forms are parser-safe or pinned with a stable
  `typescript-rust::unsupported-module-syntax` diagnostic.

## Current policy

- Script files can still share top-level `type` aliases, `interface` declarations, and function declarations across files.
- Module files keep their own top-level declarations local to the file.
- Module files do not contribute to the global script namespace.
- Module files do not see script globals under the current isolated-module policy.
- `export {}` and side-effect imports are accepted as module markers.
- Relative resolution only covers loaded `./` and `../` specifiers against already loaded `.ts` files in the current program.
- Side-effect imports may target script files or module files; they do not bind names.
- Named imports bind only the namespace that exists on the export table. Missing modules emit TS2307; missing exported members emit TS2305.
- Type-only named imports never bind value symbols.
- Exported type declarations can keep private helper types inside their defining module's local type scope when imported.
- Imports from non-relative specifiers remain intentionally unsupported and emit TS2307 or an unsupported-module diagnostic.
- Missing relative modules and missing exported names emit stable diagnostics.

## Supported module binding

- Relative specifiers: `./user`, `./user.ts`, `../models/user`, `../models/user.ts`
- Named imports: `import { User } from "./user";`
- Type-only named imports: `import type { User } from "./user";`
- Side-effect imports: `import "./setup";`
- Export-wrapped declarations: `export interface`, `export type`, `export function`, `export const/let/var`
- Named export lists: `export { User }`, `export { User as UserModel }`, `export type { User }`

## Unsupported syntax

The parser accepts unsupported module surface without panicking, but the checker does not resolve it yet:

- default imports
- namespace imports
- non-relative package imports
- re-export forms such as `export { Foo } from "./foo";`
- star re-exports
- `import = require(...)`
- `export =`
- `export default ...`

The resolver does not read from disk and does not yet handle `.js`, `.jsx`, `.tsx`, `.json`, or `.d.ts` targets.

The next phase should be chosen from compatibility-report output rather than by
expanding into package or tsconfig-path semantics by default.
