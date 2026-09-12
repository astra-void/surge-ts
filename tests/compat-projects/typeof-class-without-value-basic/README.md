# typeof-class-without-value-basic

`typeof C` where `C` is a class that has a type declaration but no value symbol
in reach: a class imported with `import type`, or a class named inside the
ambient module that declares it (`@types/node`'s `RequestListener<Request
extends typeof IncomingMessage = typeof IncomingMessage>`). surge degraded both
to the sentinel, so a `declare const x: L` typed through such a default was
silently assignable to anything, and `InstanceType<typeof C>` was lost.

The type query now stands a constructor surface over the instance type — a
construct signature returning `C` plus `prototype`, kept open so a static
member surge cannot see is never reported. That is enough for
`InstanceType<typeof C>`, for a constructor-shaped constraint, and for the
default-argument aliases `@types/node` writes. A tainted expansion is still
left degraded (see the comment at the synthesis site).
