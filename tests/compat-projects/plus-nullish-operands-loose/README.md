# plus-nullish-operands-loose

tsc's `+` runs `checkNonNullType` on its operands only when neither side is
string-like under the non-strict `isTypeAssignableToKind`, and without
`strictNullChecks` `null` and `undefined` are assignable to `string`. A
`null`/`undefined` operand of `+` is therefore never TS18050 in loose mode;
since the result kind is then decided with the strict variant (which rejects
`null` and `undefined`), a pair with no string or `any` side is TS2365. The
other arithmetic operators still check both operands and report TS18050.
