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
- declaration-only package node_modules lookup is supported for package `.d.ts` entrypoints and exact declaration subpaths
- tsconfig `paths` aliases are supported only through explicit TS7-style path targets
- `baseUrl` resolution remains unsupported/deprecated
- full package resolution remains unsupported
- full tsconfig path ecosystem features such as rootDirs/projectReferences remain unsupported
- no full declaration-file semantics, CommonJS semantics, or declaration merging
- unsupported module syntax such as `export * as Foo from "./foo"` stays parser-safe or pinned
- program-mode module visibility is limited to loaded relative `.ts` files

Design note:
- interface names are not preserved in downstream diagnostics today because the
  checker resolves them to object types before assignability and display.

## Ambient Types
Loaded `.d.ts` files contribute ambient global types which are accessible everywhere.
- exact `declare module "pkg"` blocks contribute importable ambient modules in program mode
- ambient types are loaded from project inputs, not from lib.d.ts or @types discovery
- duplicate ambient globals are first-wins / pinned rather than merged
- unsupported declaration syntax remains parser-safe and emits the pinned unsupported-declaration diagnostic
