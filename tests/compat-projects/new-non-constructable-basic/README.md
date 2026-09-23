# new-non-constructable-basic

`new` on a value whose type has no construct signatures is TS2351 (tsc's
`resolveNewExpression`), including a union with a non-constructable member.
`any`, a class and a construct-signature type are fine.
