# arrow-argument-parameter-inference-basic

A generic call whose argument is an arrow literal — `vi.fn((value: Date) =>
value.toISOString())` — bound `T` to nothing. Generic-call inference types
each argument with the expression *sketch* (not the authoritative checker, so
no contextual signature is needed), and the sketch's arrow type ignored the
written parameter annotations (every parameter was `any`) and inferred the
body without its parameters in scope, so `value.toISOString()` was an
unresolved read and the arrow's return degraded to unknown. An argument whose
type contains unknown is skipped as an inference source, and `Mock<T>` kept
its bare `T`: not callable, not assignable to the callback (the last four
`MockInstance<T>` diagnostics in the tanstack-query corpus).

The sketch now reads the annotations — a parameter that fails to resolve
stays `any`, so nothing that used to infer stops inferring — binds the
parameters into the body scope for both the expression and the block form,
and maps a written return annotation. The diagnostics that mapping emits are
discarded; the authoritative pass reports the genuine ones. A generic arrow
(`<T>(cb: () => T) => …`) keeps the untyped sketch, since its annotations name
type parameters no scope here declares.

Not pinned here: a body that calls a *global* (`(n: number) => String(n)`)
still degrades, because the sketch's call arm resolves callees from the local
table only and `String` is an object with a call signature, not a function
symbol.
