# comma-expression-value-basic

The comma operator evaluates every operand and has the type of the last. The
expression parser had no arm for it, so `(a, b)` was untyped — nothing
downstream of it was related — and errors inside its operands were never
reported. For narrowing it is the reference its right operand is
(`isMatchingReference`), so `(f(), value).inner` guards and reads
`value.inner`. An argument is reported on its effective check node, inside the
parentheses around it.
