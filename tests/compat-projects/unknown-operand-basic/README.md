# unknown-operand-basic

The `unknown` branch of tsc's `checkNonNullType`: under `strictNullChecks` an
`unknown` operand of an operator, a call, `new` or an element access is TS18046
when it is an entity name and TS2571 otherwise, and goes on as the error type.
An optional chain skips the check and reads the member from `{}`.
