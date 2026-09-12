# array-reference-parameter-inference-basic

A parameter written as `Array<T>` or `ReadonlyArray<T>` must infer `T`
element-wise from an array or tuple argument, exactly as the `T[]` shorthand
does. surge only took the shorthand path: a reference parameter fell through to
the generic-member walk, which has no object surface to match a `Type::Array`
against, so it inferred nothing.

With only the bare `item: T` parameter contributing, `addToStart(items, item)`
where `const item = 4` instantiated `T` as the literal `4` and then rejected
`number[]` against `4[]`. Both parameters contributing is what lets the common
primitive win and `T` settle on `number`.

TanStack Query's `addToStart`/`addToEnd` tests are the real-world case; it cost
six false TS2345 in that corpus.
