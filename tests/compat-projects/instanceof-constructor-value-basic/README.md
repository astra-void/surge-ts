# instanceof-constructor-value-basic

tsc's `getInstanceType`: the right operand of `instanceof` is a value, and
the candidate is the type of its `prototype` member or, failing that, what its
construct signature returns. surge resolved the operand only as a class
*name*, so a constructor-like variable narrowed nothing and every guarded
member read was a false TS2339. A member that is the candidate type matches
it, and when no member derives from the candidate but the candidate is a
subtype of the subject, the value is the candidate (`getNarrowedType`).
