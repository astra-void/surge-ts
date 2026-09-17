# conditional-expression-mismatch-anchor-basic

TypeScript 7's `elaborateError` has no case for a conditional expression. A
`cond ? a : b` that does not fit its target is related to it as one union and
reported once, at the target (the declaration name, the assignment's left
operand, the argument, the property name) — even when both branches mismatch,
or a branch is an object literal that could otherwise be elaborated. surge
reported every mismatching branch at the branch, which also changed the count.

The one place tsc does split a conditional is a value checked against a
function's *annotated* return type: `checkReturnExpression` recurses into both
branches (and nested conditionals) and anchors each on the branch itself. That
side is pinned here too, for a `return` statement and an annotated arrow's
expression body.
