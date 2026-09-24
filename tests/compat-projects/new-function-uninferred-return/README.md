# new-function-uninferred-return

Without `noImplicitAny`, `new f()` on a plain function is TS2350 only when the
function's return type is not `void` (tsc's `resolveNewExpression`). A function
declared later in the file with no return annotation returns `void` here, and a
return type not yet inferred must not be taken for another type.
