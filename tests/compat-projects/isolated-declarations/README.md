# isolated-declarations

Under `isolatedDeclarations` every type a declaration file would write must
be readable from the source alone (tsc's `pseudochecker`): a written
annotation, a literal, an object literal of such members, an `as const`
array, or a function whose single `return` is one. Anything else is reported
where the declaration emitter needed the checker (TS9007–TS9038): a variable,
property or parameter (on the initializer when it is the declaration's own
value), a function or method return, an accessor pair, an array that is not
`as const`, spreads, shorthand properties and computed names in object
literals, dynamic member names, `export default` of an expression, class
expressions and `extends` expressions, enum members that read other
declarations, properties assigned onto exported functions, and a function
expression's initialized parameter whose reused type cannot be shown to
include `undefined`. Only visible declarations are checked, and tsc reports
these alongside the program's other errors.
