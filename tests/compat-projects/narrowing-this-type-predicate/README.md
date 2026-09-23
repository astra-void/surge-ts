# narrowing-this-type-predicate

A call of a method declared `this is T` narrows the receiver
(`narrowTypeByCallExpression`): tsc reads the predicate off the call's
resolved signature (`getEffectsSignature`), so a method inherited from a base
class, one an interface inherits from a class, and one merged into an
intersection all narrow like a method the receiver's own declaration
declares. surge looked the predicate up on the receiver's own declaration
only, so `recruit.isLeader()`, `viaInterface.isLeader()` and
`combined.isLead()` narrowed nothing.

The one error, `lead` in the last branch of the `viaInterface` chain, is also
tsc's.
