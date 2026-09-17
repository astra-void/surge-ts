# arrow-expression-body-return-basic

An arrow with a written return type and an expression body is checked by
tsc's `checkReturnExpression`: a body that does not fit the annotation is
TS2322 at the body (elaborated into object literals and conditional
branches). surge only reported the elaborated forms; a plain value such as
`(): string => 1` went unreported.
