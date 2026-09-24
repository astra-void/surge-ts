# type-variable-arithmetic-operand

tsc's `checkArithmeticOperandType` asks `isTypeAssignableTo(t, number | bigint)`,
which relates a type parameter through its constraint: an unconstrained or
non-numeric type parameter operand is TS2362/TS2363 (compound assignments
included), a `number`/`bigint`-constrained one is fine. For `+`, a type
parameter that is neither number-, bigint- nor string-like is TS2365.
