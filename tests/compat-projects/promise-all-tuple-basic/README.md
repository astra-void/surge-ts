# promise-all-tuple-basic

`Promise.all([a, b])` answers a tuple of what each element awaits to — what
`const [x, y] = await Promise.all([…])` destructures — and an array of the
awaited element type for a non-literal argument. surge never reached its
`Promise.all` path, because the lib's `PromiseConstructor` is still a lazy
reference where the receiver was tested, so the call was the degradation
sentinel and nothing downstream of it was checked.

`Promise.resolve` / `Promise.reject` are typed only under
`SURGE_PROMISE_NOMINAL=1`: with `Promise<T>` collapsed to `T` their result
would be a bare value, which cannot be told from a promise where one is
expected.
