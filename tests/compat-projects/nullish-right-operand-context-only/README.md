# nullish-right-operand-context-only

Without an outer contextual type, the right operand of `??` and `||` is
contextually typed by the left operand's type (tsc's
`getContextualTypeForBinaryOperand`), which types its callbacks' parameters
(`n` is a `number`) but never requires the operand to fit: `maybe ?? { x: 5 }`
is `{ x: 1 } | { x: 5 }`. A mismatch is reported only where the whole
expression meets a declared type (TS2322 on `annotated`).
