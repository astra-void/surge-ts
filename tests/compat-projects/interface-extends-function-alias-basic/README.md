# interface-extends-function-alias-basic

`@types/express` declares its handlers as empty interfaces extending a
function-type alias: `interface RequestHandler<P = …> extends
core.RequestHandler<P, …> {}`. The interface is callable with exactly the
base's signature. surge inherited call signatures only from an *object* base
with one, so a function-typed base contributed nothing — an arrow written
against the interface lost its contextual parameter types (four false TS7006
per handler in tRPC's express adapter test).

A function-typed base now contributes its signature as the inherited call
signature; the derived interface declares nothing else. `wrongParameter` pins
that the inherited signature is really checked.
