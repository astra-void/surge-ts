# rest-parameter-alias-annotation-basic

A rest parameter is stored as the array or tuple it is written as, and the
signature comparison widens a trailing `T[]` across the positional slots (or
expands a trailing tuple into them) by matching that variant. A rest annotation
written as a library alias — `(...args: Parameters<F>)`, which is how vitest
spells every `Mock<T>` call signature through `MockParameters<T>` — resolves to
a deferred reference instead, and the comparison saw one positional parameter
holding a whole array. Every mock was reported as not assignable to any
callback, and the argument rendered as `(args?: Parameters<F>, ...args: any[])`.

The comparison now peels the rest slot. The peel is deliberately at comparison
time, not when the signature is built: a declared generic signature holds that
reference bound to a placeholder, and forcing its memo there froze the
placeholder expansion for every later instantiation (measured: zod 21→25, trpc
1134→1144).

Not pinned here (open follow-up, seen 2026-09-10): `Parameters<F>` drops the
optionality of `F`'s trailing parameters, so
`(...args: Parameters<(a: string, b?: number) => void>) => void` compares as
two required slots and is reported as not assignable to `(value: string) =>
void`; tsc accepts it.
