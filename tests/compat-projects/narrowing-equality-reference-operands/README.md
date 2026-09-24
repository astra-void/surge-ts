# narrowing-equality-reference-operands

tsc's `narrowTypeByBinaryExpression` narrows whichever operand of `==`, `!=`,
`===` or `!==` is the reference with `narrowTypeByEquality`, using the type of
the other operand whatever that operand is (flow.go). Where the comparison
holds, the reference keeps the constituents comparable to the other operand's
type, with `string` and `number` replaced by the literals that type holds;
where it fails, only a unit-typed value removes anything. surge narrowed only
against a literal (or a `const` holding one), so `y !== z` between two
bindings left both at their declared types.

`script.ts` is tsc's `equalityWithIntersectionTypes01`: `I1 & I3` and `I2`
are not comparable, so the true branch of `y !== z` being false leaves both
`never`, and every later comparison of the `else if` chain is between `never`
operands, which tsc does not report. `bindings.ts` covers the same rule in
function bodies, on either side of the comparison, in `&&`/`||` operands, and
for an `unknown` subject (`u === s` makes `u` a `string`, `u === o` an
`object`). The `bad` declarations are the intended errors.
