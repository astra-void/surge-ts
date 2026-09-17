# keyof-union-and-mapped-distribution-basic

`keyof (A | B)` is the intersection of the members' key sets (tsc's
`getIndexType` over a union): only keys every member has, where a member with a
string index answers any named key. surge degraded it to the unknown sentinel,
so every use went unchecked.

Resolving it exposed a second rule: a homomorphic mapped type whose `keyof`
operand is a type parameter distributes over a union (`instantiateMappedType`),
so `Partial<Circle | Square>` is `Partial<Circle> | Partial<Square>` and a
member only one side declares survives. A non-object constituent (`undefined`)
maps to itself.

Not covered: `keyof` of a type whose own key set is `string | number` (all
string-indexed members) still degrades, because the mapped-type and
indexed-access resolvers only accept literal keys there.
