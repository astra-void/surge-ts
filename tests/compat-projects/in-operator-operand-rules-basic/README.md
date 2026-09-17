# in-operator-operand-rules-basic

tsc's `checkInExpression`: the left operand must be assignable to
`string | number | symbol` and the right operand non-nullable and assignable to
`object`, each reported as TS2322 on the operand (TS18048 for a possibly
undefined right operand). surge checked neither.

A branded key (`"marker" & { __brand: … }`) is a valid key: an intersection
relates through its primitive constituent.
