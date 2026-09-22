# overload-implementation-hidden-basic

tsc resolves a call to an overloaded function against its overload signatures
alone: the implementation signature is not a candidate. surge folded the
implementation into the group, and since an implementation is usually written
with `any` parameters the fold accepted every argument — a call no overload
takes was silent. With the implementation out of the group, tsc's own
`chooseOverload` reporting applies: one candidate of fitting arity reports its
own TS2345/TS2554, and several report TS2769 on the argument the *last* of
them rejects — also when each argument fits *some* candidate but no candidate
fits them all (`pair(1, 2)` against `(string, number)` and `(number, string)`).
