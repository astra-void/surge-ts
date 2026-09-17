# symbol-operand-rules-basic

tsc's `checkForDisallowedESSymbolOperand`: an operand that may be a symbol is
TS2469 on that operand for a relational operator, and for `+` when the other
operand makes it a string concatenation. surge reported TS2365 on the whole
expression instead. A `+` with no string operand (`symbol + number`, `symbol +
symbol`) has no result type, so it stays TS2365 there, as pinned.
