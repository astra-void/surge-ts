# discriminant-comparable-domain-basic

tsc's `narrowTypeByDiscriminantProperty` keeps a union member only when its
discriminant type is comparable to the compared value. A member whose
discriminant is a primitive of another domain (`kind: string` against
`=== false`, `kind: number` against `=== "a"`) is ruled out exactly like a
member with a different literal; surge kept it in the matching branch, so the
guarded reads were false TS2339s. A same-domain primitive (`kind: string`
against `=== "a"`) stays in both branches.

A template without substitutions is a string-literal-like key: `` val[`kind`] ``
is the same reference as `val["kind"]` and discriminates the same way.
