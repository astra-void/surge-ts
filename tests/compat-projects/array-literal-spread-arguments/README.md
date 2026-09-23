# array-literal-spread-arguments

An array literal spread straight into a call's arguments is typed as a tuple
(tsc's `checkArrayLiteral` in `isSpreadIntoCallOrNew`), so it supplies one
argument per element and fits fixed parameters; too few elements is TS2554.
Spreading an array-typed value is still TS2556.
