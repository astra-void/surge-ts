# optional-chain-write-targets-basic

`checkReferenceExpression` worded for the statement holding the target: an
optional chain as the `for...in` (TS2780) or `for...of` (TS2781) target, or
as an object rest assignment target (TS2778), and a call as the `for...in`
target (TS2406). oxc stops parsing a file at each of these, so each sits in
its own file with its own declarations.
