# mixin-constructor-intersection-basic

tsc's `resolveIntersectionTypeMembers`: an intersection of constructor types
that mixes some in (`new (...args: any[]) => X`) constructs the intersection
of the instance types — the parameters of the constituent that is not a
mixin, its instance type intersected with every mixin's. surge kept the first
construct signature alone, so `new (typeof Mixin & typeof Base)(…)` was just a
`Mixin` and every member of `Base` a false TS2339.
