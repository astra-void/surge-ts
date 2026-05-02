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
- no package, node_modules, or tsconfig-path resolution
- no default, namespace, or star import/export semantics
- program-mode relative module visibility only for loaded `.ts` files
