# function-name-in-own-signature

tsc's binder declares every function of a scope before any signature is
resolved, so a signature may read its own function or one declared after it:
`function f(n: typeof f)`, an overload naming a later function, and
`function f(o = defaults())` with `defaults` declared below. Signatures are
collected in source order, so such a name used to be unresolved (TS2304) at
the top of a script, of a module, and of a function body alike.

A signature reading ahead now sees the signature the function has one pass
earlier — its own self-references standing at the degradation sentinel — so a
forward reference still types: the two TS2322s here read the later
function's return type, and both are `tsc` errors too.
