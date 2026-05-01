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
- no generics
- no imports, exports, or module visibility
