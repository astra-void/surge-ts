# operand-type-rules-basic

Operand checks that need the operand's type: a `void` tested for truthiness
(TS1345, in conditions, `!`, `&&`/`||` and `?:`), an `instanceof` right side
that is neither callable, constructable, a `Function`, nor carries
`[Symbol.hasInstance]` (TS2359), and a computed key not assignable to
`string | number | symbol` (TS2464).
