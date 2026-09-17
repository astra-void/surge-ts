# instanceof-left-operand-type-basic

`x instanceof C` typed as `boolean` without checking either operand, so TS2358
was never reported.

tsc's condition is `!isTypeAny(left) && allTypesAssignableToKind(left,
Primitive)`. Two exemptions in it are easy to get wrong and are pinned here:
`unknown` is deliberately **not** reported — tsc notes the related error was
already raised elsewhere — and a union is reported only when *every*
constituent is primitive, so `string | { x: number }` is accepted. A type
parameter is an object type as far as this rule is concerned.

The error goes on the left operand, not on the whole expression.
