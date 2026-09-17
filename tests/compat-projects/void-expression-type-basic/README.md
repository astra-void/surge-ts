# void-expression-type-basic

tsc's `checkVoidExpression` checks the operand and types the expression
`undefined`. surge left it unknown, so `const s: string = void 0` and reads of
a `void`-initialized binding were never related.
