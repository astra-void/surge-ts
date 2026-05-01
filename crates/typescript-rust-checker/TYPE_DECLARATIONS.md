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
- aliases use `typescript-rust::type-alias-cycle`
- interfaces use `typescript-rust::type-declaration-cycle`

Current limitations:
- no interface merging
- no `interface A extends B`
- no generics
- no methods
- no call signatures
- no construct signatures
- no index signatures
- no readonly properties
- no computed properties
- no alias/interface-preserving diagnostic display
- no imports, exports, or module visibility

Design note:
- interface names are not preserved in downstream diagnostics today because the
  checker resolves them to object types before assignability and display.
