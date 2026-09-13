# callback-rest-any-inference-basic

Two ways a callback argument failed to bind a type parameter.

**A rest parameter's element was not lined up.** Inferring from a callback
zipped the expected parameters against the actual ones positionally, so
`(...args: any[]) => any` — vitest's `vi.fn()` — bound the first expected
parameter to `any[]` and the rest to nothing. `TVariables` then kept its `void`
default and `observer.mutate(1)` reported `number` against `void`, twice in the
tanstack-query aggregate. A rest parameter stands for every position from its
own on, and it is its element the expected parameter there lines up with, so
each of `(data: TData, variables: TVariables)` binds to `any`, as tsc does.

**A callable object did not infer at all.** vitest's mock is an interface with
a call signature, not a function type, and the callback arm required the
latter. It now infers through the call signature the way a function does.

`noSourceKeepsTheDefault` and `annotatedParameterWins` are the controls: with no
source for `TVariables` the default stands and tsc's error is real, and an
annotated parameter still decides the type over the rest-any one at its
position. Both errors are expected with `@ts-expect-error`, which keeps the
project clean under the oracle.
