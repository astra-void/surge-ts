# binding-patterns-and-nested-classes

A destructured parameter or catch variable reads its names off the declared
type the way tsc's `getBindingElementTypeFromParentType` does: a property the
type lacks is TS2339 at the name (on `unknown` too), an array pattern over a
type without an iterator is TS2488 at the pattern, and an element past the end
of a fixed tuple is TS2493 unless it has a default. A class declared in a
function body has its members checked like any other class's, a computed
property name and the arguments of a parenthesized callee are reads of the
variables they name (TS2454), and `typeof C` over a class imported with
`import type` is the class itself, generic construct signature included.
