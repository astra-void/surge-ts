# inferred-type-predicate-declaration-basic

tsc's `getTypePredicateFromBody` (TS 5.5): a function with no return
annotation whose body is one `return` of a condition that splits a parameter's
type exactly in two *is* a type predicate over that parameter. surge inferred
this only for a callback written inline in `.filter(...)`; a `function`
declaration, a nested one, or a `const` arrow used as a guard
(`if (isString(x))`) or passed by reference (`xs.filter(isPresent)`) narrowed
nothing, so every guarded use was a false TS2322/TS2339.

Not predicates, and checked here as such: a condition that is not "if and only
if" (`!!v`, `typeof x === "string" && Math.random() > 0.5`), and a function
whose return type is written (`: boolean`).
