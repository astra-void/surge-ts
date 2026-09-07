# tuple-union-destructure-basic

Two halves of the same shape — a "result tuple" union, which is how tRPC's
`resolveResponse` carries a request's info or the error that replaced it.

A literal index into a *nominal alias* that peels to a tuple, or to a union of
them, reads the element. The tuple arm of the index-access check only ever saw
an unpeeled `Type::Tuple`, so `infoTuple[0]` fell through to the object arm and
was reported as a property missing from the receiver — named after the receiver
binding, since that is what the arm had in hand.

Once the elements are typed, the bindings are *dependent*: proving `infoError`
falsy rules out the union member whose first element is not nullish, and `info`
is retyped from the survivor. That is tsc's destructured-discriminated-union
narrowing. Only the source union is filtered and every sibling is re-derived
from it, so a binding whose element is identical in every surviving member keeps
the type it already had.

A union whose members are *callable interfaces* — `AnyProcedure`, three
`Procedure<…>` shapes in tRPC — is callable when they share one signature. The
union-call path only accepted bare `Type::Function` members, so calling one was
a false `TS2349`. The property-call path had no union arm at all, so the same
union reached through a member (`holder.run({ … })`) reported too.

`theOtherSideNarrowsToo` pins the opposite polarity. The single intentional
error is the last function: with no guard at all the element really is
`RequestInfo | undefined`, so binding it to a `RequestInfo` reports.
