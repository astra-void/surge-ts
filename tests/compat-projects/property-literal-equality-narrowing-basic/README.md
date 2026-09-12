# property-literal-equality-narrowing-basic

`o.p === "lit"` / `o?.p !== "lit"` narrows the property `o.p` itself, not only
the base union the discriminant narrowers filter. surge had the identifier form
(`x === "lit"`) and the discriminant form (filtering `o`'s union members by
`o.kind`), but nothing narrowed the property's own type, so after
`if (filters?.refetchType === 'none') return` the `??` chain still carried
`'none'` and TanStack Query's `invalidateQueries` reported a false TS2322
against `QueryTypeFilter`.

The guard joins the reference guards (`typeof o.p`, truthiness, `in`, …) and
composes with the discriminant test on the same condition. Its complement keeps
`undefined` — an absent property is not the literal either — and an optional
slot gets that `undefined` back in its flag rather than its type.

`ParseStatus` pins the write side: a `this.p = …` assignment is checked against
the property's *declared* type, as `o.p = …` already was. Without that, zod's
`if (this.value === "valid") this.value = "dirty"` reported the write against
the narrowed `"valid"`.
