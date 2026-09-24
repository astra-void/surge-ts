# contextual-and-comma-operands

tsc's `getContextualTypeForBinaryOperand` hands the contextual type of a
`&&` or comma expression to its right operand (and so to the value of `&&=`),
never to the left: the arrows on the right get typed parameters, the left
arrow of `(a => a) && (b => b)` is still TS7006, and a comma tail that does
not fit is still TS2322.
