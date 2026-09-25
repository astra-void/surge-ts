# this-before-super-flow

In a derived class's constructor, `this` (TS17009) and a `super.x` read
(TS17011) are errors unless every path to them runs `super(...)`
(`isPostSuperFlowNode`): branches join at labels, a `switch` case entered by
fallthrough needs both its match edge and the previous case, a conditional
expression or a short-circuit needs every operand path, a loop counts only its
entry edge (a `do` body always runs), and a parameter default runs before the
body. A reference in an arrow function belongs to the arrow, and code after a
`throw` is unreachable and silent.
