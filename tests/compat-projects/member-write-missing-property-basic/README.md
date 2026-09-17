# member-write-missing-property-basic

A write to a member the receiver does not have (`point.y = 1`) is TS2339 in
tsc, exactly as a read is. surge reported it only when the receiver was a
union, so a write to a misspelled or undeclared member of an ordinary object,
class instance, class constructor or parameter went silent.

The exception is tsc's expando declaration (`getInitializerSymbol`): `fn.x = …`
*declares* `x` when `fn` is a function declaration or a `const` initialized
with a function or arrow expression. Both are pinned as clean, beside a `let`
function expression, which is not an expando and is reported.
