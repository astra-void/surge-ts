# void-parameter-arity-basic

tsc's `getMinArgumentCount` walks a signature's trailing parameters back while
they accept `void`, so a parameter typed `void` — or a union with `void` in it,
which is what `Promise<void>`'s `resolve` takes — does not count toward the
arity a target slot has to satisfy. Without that rule surge rejected
`handler = resolve` inside a `new Promise<void>` executor, a shape tRPC's
node-http adapter tests use directly.

Only the trailing run is optional, and only `void` makes it so: `acceptsString`
and `voidThenRequired` are the two intentional errors here, and both are `tsc`
errors too.
