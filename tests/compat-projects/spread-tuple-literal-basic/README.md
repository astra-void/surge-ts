# spread-tuple-literal-basic

In a tuple context tsc's `checkArrayLiteral` flattens a spread *tuple* into
its elements and turns a spread *array* into a rest slot, so `[1, ...pair]` is
`[number, string, string]` and `[1, ...strs]` is `[number, ...string[]]`; the
literal is then related to the target as a whole. surge checked the spread as
one element against one slot (`[string, string]` against `string`) for a fixed
tuple, and typed the literal as an array against a rest tuple — every valid
literal was a false TS2322. Node positions stop lining up with slots once a
spread is present, so a failure is reported on the whole value; a nested
literal before the spread still elaborates into itself.
