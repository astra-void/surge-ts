# array-iteration-protocol-assignability-basic

surge represents an array, tuple and string as its own `Type` variant rather
than as an instance of the lib's `Array<T>` / `String` interface, so the members
those interfaces declare are answered by a synthesized surface instead of a
property map. `[Symbol.iterator]` was missing from that surface, and the
assignability arm for these sources accepted a target only when *every* member
of it was optional — so `number[]` satisfied neither `Iterable<T>` nor
`ArrayLike<T>` nor a bare `{ length: number }`, and every call through one was a
false `TS2345`.

The relation now looks each required target member up in the source's own
surface. The synthesized iterator models only what the relation reads — a
`next()` yielding `{ value: E }`, the least `IteratorYieldResult<T>` accepts —
so the element is still compared and `wrongElement` stays an error.

Two negatives are deliberately **not** pinned here, because surge and tsc agree
on rejecting them but disagree on the code:

- `Iterator<T>` / `IterableIterator<T>` targets, and an object target with a
  member no array carries. tsc elaborates to `TS2741` ("Property 'next' is
  missing"); surge reports the outer `TS2322`. That selection lives in the
  object-literal path and is unrelated to the iteration protocol.
