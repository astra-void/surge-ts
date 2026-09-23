# for-of-var-hoisting

tsc's binder declares a `var` in its function-like container, however deeply
it is nested in blocks, `if`s, loops or a `for…of` head, so a function or class
member declared before it reads it. surge collected only a file's direct
top-level declarations, so a read of a `var` nested in a top-level statement
from a function or method declared above it was a false TS2304. `wrong`
returns the hoisted `count: number` from a `string` function, which is the one
intentional error.
