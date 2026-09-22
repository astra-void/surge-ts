# global-this-member-and-inference-basic

`typeof globalThis.x` is resolved when it is *used*, not when the declaration
that names it is first collected: the global object is installed after every
ambient global, so an interface member written that way must not be interned as
`unknown` for the whole run.

The inference file pins two rules a fresh object literal argument depends on: a
type parameter inferred from several arguments takes all of their shapes (an
array of literals included), so the later argument is not excess-checked against
the earlier one — while a *written* parameter type still is; and a capture from
an optional member (`{ ext?: infer T }`) is the member's own type, without
`undefined`.
