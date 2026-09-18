# non-literal-index-key-basic

`getPropertyTypeForIndexType`: a key the receiver answers neither as a member
nor through an index signature makes the whole element access an implicit
`any` (TS7053) under `noImplicitAny` — a `string` or `number` key is not
special-cased. surge reported this only for a *literal* key that missed, so
`o[k]` with `k: string` (the shape a `for…in` body always has) was silent.

A union of literal keys still indexes member by member, and a key surge could
not model is left alone.
