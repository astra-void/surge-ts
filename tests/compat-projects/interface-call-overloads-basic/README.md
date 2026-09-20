# interface-call-overloads-basic

An interface declaring several call signatures is an overload group, as a
type literal's already was: the call resolves against the signature whose
parameters fit, so `callable("A1")` is a `string` and `callable("zz")` a
`void`. surge folded the signatures into one at parse time and lost them,
so every call read as the first one.
