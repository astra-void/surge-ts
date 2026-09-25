# element-reference-narrowing

tsc's `isMatchingReference` reads an element access with a literal or
identifier key (`results[0]`, `shapes[i]`) as a reference like a property
access, so a discriminant guard on a property reached through it narrows the
access, and `narrowTypeByDiscriminant` drops the union members a guard leaves
nothing of — through a nested property path as well.
