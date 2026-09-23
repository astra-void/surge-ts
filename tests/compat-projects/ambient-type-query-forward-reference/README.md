# ambient-type-query-forward-reference

tsc types a value on demand, so a type query in an ambient declaration reads a
variable declared after it, in the same file or a later one
(`isBlockScopedNameDeclaredBeforeUse` holds for any use in a type query or an
ambient context). surge lowered ambient variables in source order, so
`declare var S: typeof A` before `declare const A: number` was a false TS2304.
`wrong` reads `S` as `number`, which is the one intentional error.
