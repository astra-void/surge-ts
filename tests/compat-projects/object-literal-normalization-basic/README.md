# object-literal-normalization-basic

tsc's `getWidenedTypeOfObjectLiteral`: object literals that widen together —
the branches of a conditional or the elements of an array literal, written
directly in an initializer — are normalized, each gaining the names its
siblings write as `name?: undefined`. Reading such a name is `T | undefined`,
not a missing property, which surge reported as a false TS2339 on the common
`const x = c ? { a } : { a, b }; x.b`. Members keep the types they were written
with, so an `as const` discriminant still narrows. Literals reached through
variables are already widened and are not normalized.
