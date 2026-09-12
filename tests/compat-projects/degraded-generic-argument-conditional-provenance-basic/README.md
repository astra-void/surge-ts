# degraded-generic-argument-conditional-provenance-basic

`ProtectedIntersection`'s shape — `keyof TType & keyof TWith extends never ?
TType & TWith : IntersectionError<…>` — reached across a module boundary
through a generic alias, a generic function and a `typeof`.

The keys are disjoint, so tsc picks the `TType & TWith` branch and both
properties read cleanly. The pin is that surge must not pick the *collision*
branch when an operand is only `unknown` because surge failed to resolve it:
a degraded `keyof` must not simplify away inside the intersection and let
`extends never` answer from a key set that was never there. The conditional
resolver already refuses to choose a branch when the check type carries
`had_error`; this fixture pins that the signal still reaches it.

Passes with the analysis-scope upgrade on or off.
