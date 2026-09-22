# nested-function-body-basic

A `function` declared inside another body is checked like any other (it was
skipped entirely): hoisted, so it sees bindings declared after it at their
declared types, and it binds its own `this`, so a class member it names is a
plain TS2304 rather than the `this.x` hint. The fixture also pins the flow
rules those bodies exposed, each matched to tsc: a `catch` that exits leaves
the `try`'s assignments definite; a truthiness test drops `false`/`0`/`""`;
an assignment narrows a union-declared binding to the declared members the
value fits (`getAssignmentReducedType`); `||` of property guards narrows a
member reference; `const`-named `case`s decide exhaustiveness; a branded
number is still a number; an open tuple's leading element is exact; a
`unique symbol` const is removed by `!==`; an unbound predicate type
parameter is `unknown`.
