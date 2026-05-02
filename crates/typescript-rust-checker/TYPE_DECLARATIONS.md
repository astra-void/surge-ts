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
- program-mode relative imports can expose exported type declarations from loaded `.ts` files
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
- no package, node_modules, or tsconfig-path resolution
- no default, namespace, or star import/export semantics
- no re-export forms, declaration files, or CommonJS semantics
- program-mode module visibility is limited to loaded relative `.ts` files

Design note:
- interface names are not preserved in downstream diagnostics today because the
  checker resolves them to object types before assignability and display.
