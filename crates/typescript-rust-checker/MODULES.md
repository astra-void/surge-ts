# Modules

v0.56 adds a minimal import/export syntax surface and a file-level module boundary.

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
- Imports do not bind names yet.
- Exports do not participate in module resolution yet.

## Current policy

- Script files can still share top-level `type` aliases, `interface` declarations, and function declarations across files.
- Module files keep their own top-level declarations local to the file.
- Module files do not contribute to the global script namespace.
- Module files do not see exported declarations from other files through imports yet.

## Unsupported syntax

The parser accepts unsupported module surface without panicking, but the checker does not resolve it yet:

- default imports
- namespace imports
- re-export forms such as `export { Foo } from "./foo";`
- star re-exports
- `import = require(...)`
- `export =`
- `export default ...`

Imported names remain unresolved until real module binding is implemented.
