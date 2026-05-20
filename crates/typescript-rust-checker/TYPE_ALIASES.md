# Type Aliases

Type aliases now share the minimal type-declaration surface with interfaces.
See [TYPE_DECLARATIONS.md](./TYPE_DECLARATIONS.md) for the shared namespace,
collection, resolution, duplicate, and cycle rules.

This file remains the alias-focused quick reference:

Current alias scope:
- top-level `type Name = ...`
- named type references in annotations
- aliases of primitive, object, optional, and union types
- aliases of aliases
- forward references

Alias resolver model:
- aliases are collected in a top-level prepass before checking statements
- aliases are type-only and do not create value symbols
- aliases are desugared to their target `Type`
- resolution uses a stack to detect cycles
- alias names are not preserved in `Type::name()` diagnostics yet

Alias limitations:
- generic aliases now support explicit type arguments, defaults, and simple
  type-parameter substitution
- constraints are parsed and stored but are not enforced yet
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- full package resolution remains unsupported
- full tsconfig path ecosystem features such as rootDirs/projectReferences remain unsupported
- program-mode relative module visibility now includes default imports,
  namespace imports, named re-exports, type-only re-exports, and star
  re-exports for loaded `.ts` files
- v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and
  `Omit` on top of mapped types; `Required`, `Readonly`, `ReturnType`,
  `Parameters`, `Awaited`, and conditional-type-backed utilities remain
  unsupported or synthetic noise reducers. Full index signatures remain
  unsupported, while any narrow `Record<string, T>` / string-index fallback is
  confined to oracle-backed narrow paths when explicitly implemented.
- program-mode relative module visibility only for loaded `.ts` files
