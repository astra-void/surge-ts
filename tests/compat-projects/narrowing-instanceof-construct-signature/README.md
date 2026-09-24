# narrowing-instanceof-construct-signature

tsc's `narrowTypeByInstanceof` narrows by the right operand's instance type,
which `getInstanceType` takes from its `prototype` property or, failing that,
as the union of what all its construct signatures return; and
`getNarrowedType` narrows `any` (and `unknown`) to that instance type outright
in the true branch — except under `instanceof Object` and `instanceof
Function`, which leave `any` alone. surge read one construct signature of an
overloaded constructor, so `either instanceof C` narrowed nothing, and never
narrowed `any`.

Every error here is also tsc's.
