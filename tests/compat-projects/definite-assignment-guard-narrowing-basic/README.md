# definite-assignment-guard-narrowing-basic

tsc has no assignment state of its own: an unassigned local reads as
`declared | undefined` narrowed along the flow path (`getFlowTypeOfReference`),
and TS2454 is reported only while that type still holds `undefined`
(`checkIdentifier`). So a guard that strips `undefined` on an edge — `typeof`,
`instanceof`, truthiness, `=== null` (not `== null`), `!== undefined`, an
equality with a literal, a `param is T` predicate — leaves nothing to report
past it, through `!`, `&&`, `||`, `?:`, `if` and an early exit, while the
other edge still reports.

The binder folds the `true`/`false` keywords out of the flow graph
(`createFlowCondition`, through `!`/`&&`/`||`): the edge a literal condition
never takes is unreachable, and there the local reads as its declared type.
Operands of `,` and of binary operators run in order, so an assignment in one
is seen by the next.
