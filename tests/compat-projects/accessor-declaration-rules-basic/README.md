# accessor-declaration-rules-basic

tsc's `checkAccessorDeclaration`: a `get` accessor whose body can finish
without any `return` is TS2378, and a `get`/`set` pair must agree on
`abstract` (TS2676) with the getter at least as accessible as the setter
(TS2808, both reported on each accessor). surge reported none of these.
