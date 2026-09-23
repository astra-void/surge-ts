# namespace-merged-member-value

Declaration merging makes `namespace A { export class B {} export namespace B
{ … } }` one symbol `A.B` that is still the class, and a namespace merged into
a function keeps the function callable; the namespace's exports join the
value. surge's value object for `A` let the later namespace block replace the
member it merged with, so `new A.B(…)` was a false TS2351 and `A.f()` a false
TS2349.

A nested class's own value is modelled permissively, so only the members of the
merged function and of `A` itself are asserted: `A.f.missing` and
`A.notMember` are TS2339 on both sides.
