# callback-return-type-parameter-basic

A type parameter that appears only in a callback's *return* position is inferred
from the callback's body, and the body can only be typed once the callback's
*parameters* are. surge sketched every un-annotated callback parameter as `any`
before inference ran, so `read<TResult>(map: (value: TValue) => TResult)` handed
`(value) => value.length` inferred `TResult` from an `any` body: the call
returned `any` and every diagnostic downstream of it disappeared. Such an
argument is context-sensitive and now waits for a second inference pass, which
types it with the parameters the other arguments have already pinned — the
enclosing interface's or class's own arguments included.

The last case is the same inference through the awaited model: surge models a
resolved `Promise<T>` as its awaited `T`, so a written `Promise<TData>` is
matched against the awaited value itself, which the member-for-member walk over
a generic reference could not line up with a primitive.

Each closing case is paired with its negative: the type parameter is inferred
concretely, so the mismatched annotation next to it must still be rejected.
