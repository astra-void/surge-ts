# primitive-assertion-overlap-basic

tsc's `checkAssertionDeferred` rejects `x as T` when neither side is
comparable to the other, after widening the source's literals (TS2352). surge
emits it where both sides are primitives or literal unions of them, which
needs no structural expansion; between object types it stays withheld (see
`assertion-overlap-basic`).
