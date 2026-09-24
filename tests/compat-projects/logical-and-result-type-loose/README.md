# logical-and-result-type-loose

tsc's `&&` result (`checkBinaryLikeExpression`): when the left may be truthy,
the definitely-falsy part joined to the right comes from the left under
`strictNullChecks` but from the right's base type without it, so `text && 1`
is `number`; a left that is never truthy is the whole result. `text && ""`
is `string` and does not fit `number`.
