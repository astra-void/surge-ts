# disjoint-protected-intersection-basic

The positive control for `ProtectedIntersection`. `A` and `B` have disjoint
keys, so `keyof A & keyof B` is `never`, the conditional takes `A & B`, and
both properties must be readable — a provenance fix must not degrade every
`ProtectedIntersection` into the error branch.

The second half pins the opposite direction: when the key sets genuinely
overlap the collision branch *is* selected, and its template-literal error type
stays assignable to `string`.
