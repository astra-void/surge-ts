# accessor-parameter-grammar-basic

tsc's `checkGrammarAccessor` parameter rules on class, object-literal and
interface accessors: a getter with parameters (TS1054), a setter without
exactly one (TS1049), a setter's rest (TS1053), optional (TS1051) or
defaulted (TS1052) parameter, and `checkParameter`'s TS2784 for a `this`
parameter on either accessor — reported even when the count is also wrong.
A setter returning a value is TS2408 (`checkReturnStatement`).
