# constructor-guard-definite-assignment

`x.constructor === C` narrows `x` to what `C` constructs on its equal edge
(tsc's `narrowTypeByConstructor`), which is never the `undefined` of an
unassigned local, so reads there are not TS2454. The inequality's other edge
narrows the same way; `Object` and `Function` narrow nothing, and the
inequality edge itself keeps `undefined`.
