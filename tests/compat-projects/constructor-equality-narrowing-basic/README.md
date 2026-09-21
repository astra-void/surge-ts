# constructor-equality-narrowing-basic

tsc's `narrowTypeByConstructor`: `x.constructor == C` (or
`x["constructor"]`, either way round) leaves, in the branch where it holds,
only the union members `C` itself constructs — a class instance of exactly
that class, since a subclass has its own constructor, a primitive for its
wrapper, an array for `Array`. surge narrowed nothing, so the guarded member
reads were false TS2339s. The other branch is not narrowed.
