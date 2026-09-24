# compound-assignment-not-definite

tsc's `getTypeAtFlowAssignment` gives the target of a compound assignment
(`AssignmentKindCompound`: `+=`, `**=`, `<<=`, …) the flow type it had before,
so it never makes an unassigned variable definitely assigned: every later
read is still TS2454. A plain or logical assignment (`=`, `??=`, `||=`, `&&=`)
does assign.
