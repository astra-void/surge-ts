# destructuring-assignment-basic

`[a, b] = source` and `({ key: a } = source)` were dropped by the parser, so
none of their writes was checked. tsc's `checkDestructuringAssignment` relates
each element or property of the source to its target and reports on the target:
a mismatch (TS2322), a constant target (TS2588), an unresolved one (TS2304).

Each identifier target is now lowered to an ordinary assignment of the element
or property it reads, with a default applied as `??` the way a binding pattern
is. A swap and a defaulted element are pinned as clean.
