# self-reference-and-placement-basic

Declarations that reach themselves and syntax in the wrong place:
a private name on both the static and instance side (TS2804); variables whose
annotations reach each other through an eager `typeof` (TS2502 — a member of
an object type or a signature is resolved on demand and does not count); a
type parameter constrained, through its list, by itself (TS2313 — one that
only runs into another cycle is not reported); `unique symbol` outside a
`const`, a `static readonly` property or a `readonly` signature (TS1330,
TS1331, TS1332, TS1335); an unannotated rest parameter (TS7019, from its
`...`); a decorator on a plain function's parameter (TS1206); `import =
require` inside a namespace (TS1147); a parameter initializer reading itself
(TS2372) or a later parameter (TS2373) — a later parameter read by a nested
function is in scope and is not an unresolved name; and a destructuring
rename in a signature with no body (TS2842), which is not also an implicit
`any`.
