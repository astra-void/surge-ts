# Type Declarations

Minimal type declarations are shared by top-level `type` aliases and
top-level `interface` declarations.

Shared namespace rules:
- both declarations are type-only and do not create value symbols
- both are collected in a top-level prepass before statement checking
- both support forward references
- both live in the same type-declaration namespace
- duplicate type declarations emit `TS2300`
- first declaration wins and later duplicates do not replace the original

Resolution model:
- aliases are desugared to their target `Type`
- interfaces are lowered to `Type::Object(ObjectType { ... })`
- aliases and interfaces can carry type parameters, and explicit type arguments
  are substituted into the declaration body before lowering
- type parameter defaults are supported for trailing omission cases
- constraints are parsed and stored but are not enforced yet
- declared type parameters resolve to `unknown` inside generic function
  signatures until a fuller instantiation model exists
- duplicate type parameter names emit a stable custom diagnostic and do not
  change the first-wins lowering policy
- aliases use `typescript-rust::type-alias-cycle`
- interfaces use `typescript-rust::type-declaration-cycle`
- program-mode relative imports and re-exports can expose exported type declarations from loaded `.ts` files
- imported type declarations keep the defining module's local type scope for private helper references

Current limitations:
- no interface merging
- no `interface A extends B`
- no generic inference
- no methods
- no call signatures
- no construct signatures
- no index signatures
- no readonly properties
- no computed properties
- no alias/interface-preserving diagnostic display
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- full package resolution remains unsupported
- full tsconfig path ecosystem features such as rootDirs/projectReferences remain unsupported
- no full declaration-file semantics, CommonJS semantics, or declaration merging
- unsupported module syntax such as `export * as Foo from "./foo"` stays parser-safe or pinned
- program-mode module visibility is limited to loaded relative `.ts` files

Design note:
- interface names are not preserved in downstream diagnostics today because the
  checker resolves them to object types before assignability and display.

## Type Operators
Type operators provide a parser-safe foundation for common compatibility patterns.

`typeof value`:
- Resolves to the inferred type of a top-level or in-scope value symbol
- If the value symbol is unresolved, emits `TS2304` or defaults to `unknown`

`keyof T`:
- Extracts the property names of an object or interface type into a string literal union
- Optional properties still contribute their names
- `keyof typeof constObject` maps are supported
- Unresolved or unsupported targets (primitives, template literal types, index signatures, etc.) fallback to `unknown` without exact TypeScript semantics

Current limitations:
- full indexed access types (e.g., `T[K]`)
- mapped types (e.g., `{ [K in keyof T]: T[K] }`)
- conditional types
- generic constraint enforcement on `keyof`
- `typeof import("pkg")`
- namespace and class constructor `typeof` semantics
Loaded `.d.ts` files contribute ambient global types which are accessible everywhere.
- exact `declare module "pkg"` blocks contribute importable ambient modules in program mode
- ambient types are loaded from project inputs, not from lib.d.ts or @types discovery. v0.72/v0.72.1 uses synthetic built-ins, not physical `lib.d.ts`. `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics, while utility types mostly suppress TS2304 without mapped/conditional semantics. `noLib: true` disables these synthetic built-ins. DOM, Node, `@types`, and true lib loading remain unsupported.
- duplicate ambient globals are first-wins / pinned rather than merged
- unsupported declaration syntax remains parser-safe and emits the pinned unsupported-declaration diagnostic
