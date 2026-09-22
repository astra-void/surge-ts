# loop-assignment-flow-basic

A loop head sees the union of what flows in and what the body leaves on
the back edge, and an assignment narrows the declared union to the members
the value can inhabit (`getAssignmentReducedType`). surge widened every
loop-assigned binding to its declaration and kept a `null` narrowing
through `d ??= {…}`, so `d` stayed `null` after the write.
