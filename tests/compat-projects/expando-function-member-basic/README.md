# expando-function-member-basic

tsc binds `fn.x = value` as a declaration of `x` on `fn` when `fn` is a
function declaration or a `const` holding a function: the value becomes
`{ (…): R; x: typeof value }` for every reader — after the write, in a
function declared earlier in the file (the member is hoisted with its
function), and in an importer (`Card.Header = Header`, then `Card.Header()`).
Several writes of one name declare the union of their types. surge exempted
the write itself but never gave the function the member, so every read was a
false TS2339.

Not expandos, and still TS2339: a `let` holding a function, a class, a plain
object literal.
