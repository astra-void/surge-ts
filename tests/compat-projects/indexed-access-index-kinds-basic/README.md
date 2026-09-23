# indexed-access-index-kinds-basic

An indexed access type `T[K]` that is not deferred as generic resolves through
tsc's `getIndexedAccessTypeOrUndefined` and `getPropertyTypeForIndexType`
(checker.go). A union index distributes over its members (`boolean` inside a
larger union is `false | true`); each key is looked up as a property first —
a tuple names its elements, a union receiver reads the property from every
member, and a tuple past its named elements reads its rest elements — and then
through the applicable index signature (`isApplicableIndexType`): a string
signature answers numeric keys, a number signature answers numeric string
literals and `` `${number}` ``, and an enum member key is its literal value.

When nothing answers, the error follows the key's kind: a string or number
literal is a missing property (TS2339, TS2493 past a fixed tuple's end),
`string` or `number` finds no matching index signature (TS2537), and any other
key cannot index at all (TS2538).

These errors belong to the written node only (`accessNode`). An access naming a
type parameter is deferred as generic and validated by
`checkIndexedAccessIndexType` against the parameter itself, so `src/generic.ts`
checks that a mapped key constrained to `keyof T`, a method's own
`K extends keyof Props` and a generic `[keyof T]` index report nothing, that an
instantiation (`First<[]>`) reads its result silently, and that `T["name"]` on
an unconstrained `T` is still TS2536.

The `Bad*` aliases, the `bad*` constants and `Direct` are the intentional
errors; all are `tsc` errors too.
