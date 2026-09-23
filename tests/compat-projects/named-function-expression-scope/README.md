# named-function-expression-scope

tsc's `resolveName` reaches a function expression's own name at the
`FunctionExpression` node, past the function's parameters and locals
(`case KindFunctionExpression`), so the name is in scope in the expression's
own signature and body and nowhere else. surge's lowering dropped the name, so
every self-reference — `var a = function f() { return f; }`, an IIFE, a
callback argument, a template substitution — was reported as missing
(TS2304).

The name is the function itself: a write to it is TS2630, it does not leak out
of the expression (TS2304), and a method's name is a property rather than a
binding (TS2304); all three are `tsc` errors too.
