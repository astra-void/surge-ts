# array-pattern-iterability-basic

An array binding pattern reads its source through the iteration protocol, so
tsc reports a non-iterable source once, on the pattern (TS2488), and types
every element `any`. surge lowered each element to `source[i]` and reported
that access instead (TS7053 on an object), or nothing for a primitive.
