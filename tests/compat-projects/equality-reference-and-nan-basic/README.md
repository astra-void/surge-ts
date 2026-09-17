# equality-reference-and-nan-basic

Two equality checks in tsc's `checkBinaryLikeExpression` need only syntax:

- an object, array or `function` literal operand makes the comparison constant,
  because objects compare by reference (TS2839, `false` for `==`/`===` and
  `true` for `!=`/`!==`); an arrow function is not on tsc's list;
- comparing with the global `NaN` is constant too (TS2845); a parameter named
  `NaN` is not the global.

surge reported neither. Class-expression and regular-expression operands, which
tsc also lists, are not modelled yet.
