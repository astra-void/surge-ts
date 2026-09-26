# generator-yield-types-nonstrict

Without `strictNullChecks`, a generator whose every `yield` yields a widening
`undefined` or `null` implicitly has an `any` yield type: TS7055 on a named
declaration, TS7025 on an expression. Iterating a generator whose `TNext` does
not accept `undefined` is TS2763 (`for...of`/spread), TS2766 (`for await`) or
TS2764 (`yield*` into a generator whose `TNext` it does not accept).

Not covered yet: a generator class method gets no TS7055 in surge (checked
2026-09-26).
