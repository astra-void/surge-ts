# grammar-comma-operator-basic

`TS2695` fires when a comma operand's value is discarded and evaluating it
cannot have done anything — tsc's `isSideEffectFree` list.

`propertyLeft` and `callLeft` pin the two shapes that stay quiet: a property
access can run a getter, and a call is a call. The `for` head pins the common
legitimate comma, whose operands are assignments.
