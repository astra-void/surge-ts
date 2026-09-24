# function-without-return-is-void

A function whose body returns nothing — only statements, a `throw`, or
returns inside nested functions — has the return type `void`
(`getReturnTypeFromBody`), so using its result as a `number` is TS2322, and
without noImplicitAny it can be called with `new` (only a non-`void`
function is TS2350 there).
