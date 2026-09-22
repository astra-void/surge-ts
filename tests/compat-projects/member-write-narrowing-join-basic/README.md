# member-write-narrowing-join-basic

tsc narrows a property reference on assignment by reducing its *declared*
type with what was assigned (`getAssignmentReducedType`). surge reduced the
slot's current type, so after `if (!o.c)` narrowed it to `undefined` the write
narrowed nothing and the join after the `if` still held `undefined` — a false
TS2532/TS18048 on the idiomatic lazy initialisation. `this.<member> = v` never
narrowed at all (its statement took an immutable scope).

A class merged with a same-named interface is one declaration; its own
`private` members were reported inaccessible inside it (TS2341), because the
merged record carries the interface's name span while the enclosing class
recorded its own.
