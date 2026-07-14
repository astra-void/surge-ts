# Automatic `@types` discovery, `types`, and `typeRoots`

This documents how project mode resolves `compilerOptions.types` /
`compilerOptions.typeRoots` and loads type-package declarations. The logic lives
in the `surge-ts` facade's `package_declarations` module and is wired from
`Project::check`.

## Important: TypeScript 6.0 removed implicit `@types` inclusion

The pinned oracle is **TypeScript 7.0.2** (the native compiler); TypeScript
6.0.3 is retained as the `typescript-6` benchmark alias. Starting with
TypeScript 6.0, and unchanged in 7.0, the compiler does **not** automatically
include every visible `node_modules/@types/*` package when
`compilerOptions.types` is absent. Verified against both the 7.0 oracle and the
6.0.3 baseline, which behave identically:

| Config | tsc 6.0.3 |
| --- | --- |
| `types` absent, local `@types/node` present | `process` **not** found (TS2580/TS2591) |
| `typeRoots: ["./x"]` but no `types` | nothing auto-included |
| `types: ["node"]` | node included |
| `types: ["*"]` | **all** packages under the type roots included |

The mechanism is `getAutomaticTypeDirectiveNames` in the TS source:
`options.types ?? []` unless `types` contains the `"*"` wildcard, in which case
each `"*"` expands to the packages discovered under the effective type roots.

So the oracle-faithful way to get "discover all visible `@types`" is the explicit
`compilerOptions.types: ["*"]` wildcard. The fixtures under
`tests/compat-projects/auto-types-*` use it for this reason.

## Behavior

`resolve_type_packages(types, type_roots, root_dir)`:

- **`types` absent (`None`) or `types: []`** — include nothing.
- **`types` without `"*"`** — include only the listed packages (existing
  configured-`types` behavior).
- **`types` containing `"*"`** — expand the wildcard to every package discovered
  under the effective type roots, preserving any other literal entries; the list
  is deduped in order.

### Effective type roots (`getEffectiveTypeRoots`)

- If `typeRoots` is set → exactly those directories (the default
  `node_modules/@types` chain is **not** consulted).
- Otherwise → every ancestor `node_modules/@types` directory, nearest first
  (existence is checked lazily while scanning/resolving).

### Wildcard discovery (`getAutomaticTypeDirectiveNames`)

Each effective type root is scanned once for immediate sub-directories. Skipped:
dot-prefixed directories and "not needed" stub packages (a `package.json` whose
`typings` field is JSON `null`). Names are the raw directory base names — i.e.
the mangled form (`scope__pkg`) for scoped `@types` packages.

### Name resolution and mangling (`getCandidateFromTypeRoot`)

Each directive name resolves nearest-root-first; the first hit wins (nearest
package wins on duplicates). Scoped names are mangled (`@scope/pkg` →
`scope__pkg`) **only** under `@types` roots; custom `typeRoots` use the name
verbatim. Entrypoint resolution stays narrow: `package.json` `types` / `typings`
/ exact `exports["."].types` / `index.d.ts`.

### Diagnostics

- Explicit (non-wildcard) `types` entries that resolve nowhere emit **TS2688**
  (`Cannot find type definition file for '...'`). Wildcard-discovered packages
  never produce TS2688 (they come from directories that exist).
- The node install hint follows TS's `usesWildcardTypes` branch: **TS2580**
  (`... npm i --save-dev @types/node`) when `types` used `"*"`, otherwise
  **TS2591** (`... and then add 'node' to the types field`). The checker detects
  the wildcard via a `"*"` sentinel kept in `CheckerOptions.types`
  (`CheckerOptions::types_uses_wildcard`).

## Globals contribution

Included packages contribute ambient globals/modules. `@types/*` files are
dependency declarations and are gated into the ambient global scope by
`is_configured_types_global_file` (matching `/@types/<mangled>/`). Packages under
custom `typeRoots` are not under `node_modules`, so they classify as root
declarations and contribute globals directly.
