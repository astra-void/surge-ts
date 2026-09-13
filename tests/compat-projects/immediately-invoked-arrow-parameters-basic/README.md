# immediately-invoked-arrow-parameters-basic

An arrow invoked on the spot — `((table) => …)(f.field.table)` — had nothing to
type its parameters from, so under `noImplicitAny` every one of them reported
`TS7006`. tsc types them from the call's own argument at the same position.

The callee expression was already in the AST (`ParsedExpression::ExpressionCall`
carries it); only the checking treated the arrow as a standalone expression with
no expected type. It is now checked against a signature built from the
arguments, so drizzle's five dialect files stop reporting.

`anAnnotatedParameterIsUnchanged` and `aFunctionExpressionStillWorks` are the
controls for the two paths that already worked — the recovery is skipped
entirely when every parameter is annotated.

`theParameterReallyHasTheArgumentsType` is the intentional error and pins that
the parameter really takes the argument's type rather than `any`: `t.name` is a
`string`, so assigning the call's result to `number` reports. An `any` parameter
would make it silent.
