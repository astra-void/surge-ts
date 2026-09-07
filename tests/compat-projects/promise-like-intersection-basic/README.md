# promise-like-intersection-basic

surge models `Promise<T>` / `PromiseLike<T>` as its awaited `T` — an implicit
await everywhere — so `PromiseLike<void> & { pull(): void }` reaches the
intersection merge as `void & {…}` with the promise's own `then` already gone.
The merged object surface was closed, so `then` on a value written against that
type was reported as an excess property. tRPC's subscription test writes exactly
this puller.

A `void` operand is treated as the same kind of loss the degradation sentinel
already marks: the surviving object stays open. `objectSurfacesStayChecked` is
the intentional error, pinning that an ordinary object target still rejects an
excess property.
