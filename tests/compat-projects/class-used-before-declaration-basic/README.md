# class-used-before-declaration-basic

A class binding is in its temporal dead zone until its declaration is
evaluated, so a module-level reference that runs earlier is TS2449. Nothing
reported it.

What makes this rule easy to get wrong is everything it does **not** report,
and all of it is pinned here. tsc decides legality with
`isUsedInFunctionOrInstanceProperty`, which walks up from the use site and
quits at the first function-like ancestor: a function expression, an arrow, a
function declaration and an object-literal method all defer their bodies, so an
early reference inside any of them is fine. A type position is exempt for the
same reason (`isInAmbientOrTypeNode`) — including the `typeof Later` half of a
declaration whose *value* half is still reported. An ambient `declare class`
has no evaluation to be early of. An `export { X }` clause names a binding
without reading it.

The walk here mirrors that shape rather than tracking a "am I inside a
function" flag: it never descends into a function or arrow body, which costs
nothing on the identifier path (it runs once per module statement) and makes
the type-position exemption fall out for free, since annotations are a separate
tree from the expressions it visits.

The error anchors on the identifier, not on the enclosing `new` expression.
