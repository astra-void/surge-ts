# annotated-void-return-basic

A *contextual* `void` return accepts anything — `const cb: () => void = () => 1`
is fine, which is what lets `forEach(x => list.push(x))` type-check. A
*written* `: void` is an ordinary annotation, and tsc's `checkReturnExpression`
relates the returned value to it: `(): void => 1` is TS2322 on the body. surge
skipped the return check for every `void` return type on an arrow function or
object-literal method, annotated or not, so the written form was silent (a
function declaration and a class method were already checked).
