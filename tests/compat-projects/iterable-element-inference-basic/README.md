# iterable-element-inference-basic

A type parameter written as the element of an `Iterable<T>` parameter is
inferred from an array or tuple argument, the same way `T[]` and `Array<T>`
already were. surge represents an array as `Type::Array`, which has no object
surface for the interface-member walk to match against, so before this the
parameter contributed no candidate at all and `T` stayed uninferred.

`Object.fromEntries(entries: Iterable<readonly [PropertyKey, T]>)` is the shape
that motivated it: the `readonly [string, V][]` a caller hands it has to reach
`T` for the result to keep the index signature tsc gives it.

`ArrayLike<T>` and `ConcatArray<T>` are deliberately *not* in the set: inferring
through them exposes a separate gap — an array argument is not assignable to
either interface in surge — which turns a silent call into a false `TS2345`.
