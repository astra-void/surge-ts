# arrow-unit-return-widening-basic

An arrow with an expression body kept the literal type of its body as its
return type, so `() => 1` returned `1` and `ReturnType<typeof f>` rejected
every other number. tsc's `getReturnTypeFromBody` widens a *unit* return type
(a single literal) unless the contextual return type is literal-like for it
(`isLiteralOfContextualType`). A block body and a function declaration already
widened.

What does not widen is pinned beside it: a union of literals
(`c ? "a" : "b"`), an annotated literal return, and a literal contextual return
type.
