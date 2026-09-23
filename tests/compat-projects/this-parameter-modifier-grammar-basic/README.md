# this-parameter-modifier-grammar-basic

The parser's TS1433: a decorator or modifier on a `this` parameter, anchored
on the first of them — in a function, a constructor (where oxc accepts the
modifier and fails at `this`), with two modifiers, and with a decorator. It is
a parse error, so each case sits in its own file: oxc gives up on the file at
it, and tsc reports no semantic diagnostic for the program.
