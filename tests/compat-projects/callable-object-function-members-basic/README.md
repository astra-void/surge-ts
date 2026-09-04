# callable-object-function-members-basic

A type carrying a call or construct signature *is* a `Function`, so it answers
`name`, `length`, `call`/`apply`/`bind` and `toString`. surge modelled those on
`Type::Function` only, so an object form — a `new (...) => T` alias, a class's
static side, an interface with a call signature — reported TS2339 for every one
of them.

The last declaration keeps the fallback honest: the members resolve to their
real types, so reading `Widget.name` as a `number` still reports.
