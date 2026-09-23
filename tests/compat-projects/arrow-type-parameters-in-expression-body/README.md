# arrow-type-parameters-in-expression-body

A generic arrow's type parameters are in scope everywhere inside it, including
a type assertion in its expression body — tsc's `resolveName` finds them
lexically. surge's inference sketch of an arrow (used for the element types of
array literals and the values of object literals) typed the body without them,
so `U` in `<T, U>(x: T) => x as unknown as U[]` was a false TS2304. `V`, which
nothing declares, is still TS2304.
