# any-never-target-basic

tsc's `isSimpleTypeRelatedTo` rejects a `never` target before the rule that
lets `any` relate to everything under assignability (relater.go), so only
`never` itself is assignable to `never` — not `any`, `any[]` to `never[]`, a
function returning `any` to `() => never`, or an `any` argument to a `never`
parameter. surge let `any` through, which also decided the `IsNever` idiom
`[T] extends [never]` the wrong way for `any`: expect-type's `IsAny<any>` came
out `false`, and `expectTypeOf(x).toBeAny()` resolved to a non-callable object.

A naked `any` check type (`T extends never ? … : …`) is not the relation's
business: tsc yields both branches there, so either literal is accepted.
