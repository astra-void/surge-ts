# iife-contextual-parameters

An immediately invoked function's unannotated parameters are typed from the
call's arguments (tsc `getContextuallyTypedParameterType` over
`getEffectiveCallArguments`): a spread tuple supplies one argument per element,
a rest parameter takes the remaining arguments (or the spread array), and a
parameter with no argument is `undefined` or its widened initializer. Such a
parameter is optional in the call (`isOptionalParameter`), so
`((first, second) => second)(1)` and `((...none) => none.length)()` are fine;
an annotated parameter stays required (TS2554).
