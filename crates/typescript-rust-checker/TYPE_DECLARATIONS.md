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
- v0.84.11 extends that lowering narrowly for imported interface bases, same-file forward references, and string-index fallback objects, so `interface extends` over already-loaded relative modules can resolve without package discovery. Full index signatures remain unsupported, while any narrow `Record<string, T>` / string-index fallback stays confined to oracle-backed narrow paths when explicitly implemented; `Required`, `Readonly`, `ReturnType`, `Parameters`, `Awaited`, and conditional-type-backed utilities remain unsupported or synthetic noise reducers

Current limitations:
- no interface merging
- narrow interface `extends` over imported object/interface types is supported; full declaration merging is still unsupported
- no generic inference
- no methods
- no call signatures
- no construct signatures
- no full index signatures; the checker only models a narrow string-index fallback for fixture-backed paths and synthetic `process.env`
- no readonly properties
- no computed properties
- no alias/interface-preserving diagnostic display
- explicit `paths` aliases and declaration-only package entries share the same internal resolved module map
- `baseUrl` resolution remains unsupported/deprecated
- full package resolution remains unsupported
- full tsconfig path ecosystem features such as rootDirs/projectReferences remain unsupported
- no full declaration-file semantics, CommonJS semantics, or declaration merging
- unsupported module syntax such as `export * as Foo from "./foo"` stays parser-safe or pinned
- full index signatures remain unsupported; narrow string-index fallback behavior only appears in oracle-backed fixture paths when explicitly implemented
- program-mode module visibility is limited to loaded relative `.ts` files

Design note:
- interface names are not preserved in downstream diagnostics today because the
  checker resolves them to object types before assignability and display.

## Type Operators
Type operators provide a parser-safe foundation for common compatibility patterns.

`typeof value`:
- Resolves to the inferred type of a top-level or in-scope value symbol in a narrow type-position subset
- If the value symbol is unresolved, emits `TS2304` or defaults to `unknown`

`keyof T`:
- Extracts the property names of an object or interface type into a string literal union in a narrow type-position subset
- Optional properties still contribute their names
- `keyof typeof constObject` maps are supported
- Unresolved or unsupported targets (primitives, template literal types, index signatures, etc.) fallback to `unknown` without exact TypeScript semantics
- v1.1 supports narrow generic indexed access after concrete substitution, including `T["key"]`, `T[K]`, and `T[keyof T]` when the receiver/key have been substituted to concrete types. Fully unresolved generic indexed access and constraint enforcement remain unsupported.
- Narrow indexed access types (`T["K"]`, `T[keyof T]`, tuple numeric literal index) are supported. Unresolved index keys correctly emit cascading `TS2538` diagnostics under the `tsc` profile.
- Mapped types (`{ [K in keyof T]: T[K] }` and `{ [K in keyof T]?: T[K] }`) are supported. Homomorphic mapped types and optional mapped properties map over string-literal keys. Generic mapped aliases are supported after concrete substitution. Key remapping, conditional types, template literal types, index signatures, readonly mapped semantics, modifier arithmetic, generic inference, `@types`, and modifiers beyond bare `?` remain unsupported. The generated default-lib subset supplies the ambient core/DOM globals used by the checker, but physical `lib.d.ts`, Node discovery, and full lib.d.ts parity remain future work. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of that mapped-type foundation: supported key shapes are string-literal unions, and supported sources are concrete object/interface shapes. This is still not full TypeScript utility-type support.

Current limitations:
- fully unresolved generic indexed access types and generic constraint enforcement on `keyof`
- mapped type modifiers `readonly`, `-readonly`, `-?`, `+?`
- key remapping `as SomeRemap<K>`
- conditional types
- generic constraint enforcement on `keyof`
- `typeof import("pkg")`
- namespace and class constructor `typeof` semantics
Loaded `.d.ts` files contribute ambient global types which are accessible everywhere.
- exact `declare module "pkg"` blocks contribute importable ambient modules in program mode
- ambient types are loaded from project inputs, and v0.85 adds a generated default-lib foundation on top of the project input surface. It does not load the full official TypeScript lib files at runtime; instead it generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity, Node, and `@types` discovery remain future work.
- duplicate ambient globals are first-wins / pinned rather than merged
- unsupported declaration syntax remains parser-safe and emits the pinned unsupported-declaration diagnostic
