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
  `Omit` on top of mapped types.
- A narrow conditional-type evaluator now backs `Check extends Extends ? True :
  False`. It evaluates concretely when both sides are concrete (a single
  assignability test selects the branch) and distributes over unions when the
  check type is a naked type parameter. On top of it, `Exclude<T, U>`,
  `Extract<T, U>`, and `NonNullable<T>` are real conditional-type aliases (no
  independent synthetic resolver). `never` is modeled as the empty type: it is
  assignable to everything, nothing is assignable to it, and it is dropped from
  unions (`T | never === T`). `ReturnType` and `Parameters` stay as narrow
  synthetic lowerings over concrete function types (no `infer`).
- Template literal types are supported as a narrow evaluator: a template whose
  interpolations all resolve to finite string/number/boolean literal unions
  expands to the deduped cartesian-product string-literal union (e.g.
  `` `/${"users"|"posts"}/${"new"|"edit"}` ``). This works after explicit
  generic substitution and over `keyof` results. Broad or unresolved
  interpolations (`` `id:${string}` ``) degrade to `string` rather than
  cascading; this under-reports relative to tsc but never produces a false
  positive. Recursive expansion, `infer`/pattern matching inside templates, and
  the intrinsic string utilities (`Uppercase`, etc.) remain unsupported.
- Still unsupported in this slice: `Required`, `Readonly`, `Awaited`, full
  conditional-type inference, arbitrary/nested `infer`, recursive conditional
  evaluation, and key remapping in mapped types. An
  unsupported conditional (e.g. one containing nested `infer`) degrades to
  `unknown` rather than cascading. Full index signatures remain unsupported,
  while any narrow `Record<string, T>` / string-index fallback is confined to
  oracle-backed narrow paths when explicitly implemented.
- program-mode relative module visibility only for loaded `.ts` files
