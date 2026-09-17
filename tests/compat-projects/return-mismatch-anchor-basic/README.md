# return-mismatch-anchor-basic

tsc's `checkReturnStatement` relates the returned value with the `return`
statement as the error node, so a value that does not fit is reported at
`return`; only an elaborated literal (an object property, a conditional
branch) points inside it. surge reported at the expression.
