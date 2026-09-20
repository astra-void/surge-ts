# nullish-operand-basic

tsc's `checkNonNullType` on operator operands and call/`new` targets
(`reportObjectPossiblyNullOrUndefinedError`): the `null` keyword and the
identifier `undefined` are TS18050, an entity name is TS18048, and a
parenthesized operand is the unnamed TS2531/TS2532 anchored at its
parentheses. A call target uses the invocation wording (TS2721/TS2722); a
`for…in` right-hand side must be an object once `null`/`undefined` are removed
(TS2407); `new` on a call-only signature is `any` (TS7009). `+` decides its
result kind before reporting, so one string operand makes it a concatenation.
