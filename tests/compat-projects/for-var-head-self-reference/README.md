# for-var-head-self-reference

A `var` in a `for…in`/`for…of` head is hoisted, so the expression the loop
iterates reads the variable the head declares. Its type depends on that
expression (`getTypeForVariableLikeDeclaration`), so it is circular there and
reads as `any` (`reportCircularityError`; TS7022 under `noImplicitAny`), which
satisfies the `for…in` operand check. A variable an earlier declaration
already typed (`declaredFirst: string`) is not circular, and iterating it with
`for…in` is TS2407. surge gave a hoisted head its key type `string` at the top
level (a false TS2407) and left it out of scope in a function body (a false
TS2304).
