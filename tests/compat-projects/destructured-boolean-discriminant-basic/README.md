# destructured-boolean-discriminant-basic

`const { done, value } = reader.read()` narrows `value` by a truthiness test
of `done`: a boolean-literal discriminant partitions
`{ done: false; value: T } | { done: true; value?: undefined }`. surge's
dependent-destructuring test treated only `undefined`/`void`/`never` as falsy,
so `if (!done)` / `if (done) break` narrowed nothing and `value` stayed
`T | undefined` (a false TS2365 on `buffer += value`, the stream-reading loop).
The truthy branch keeps the `done: true` member, whose `value` is `undefined`.
