# var-redeclaration-identity-basic

A `var` has one type, its first declaration's. Every later declaration must
be *identical* to it (tsc's `isTypeIdenticalTo`, TS2403 otherwise): an alias,
a mapped type over a closed key set, a reordered union or intersection, and a
callable object literal all denote the same type as their expansion, while
optionality and `readonly` must agree. surge compared each declaration with
the one before it using structural equality of its internal representation,
so equivalent spellings were false TS2403, a third declaration was checked
against the second, and the binding took the last declaration's type.
