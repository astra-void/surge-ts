# package-reexport-array-index-basic

A value reaching the caller through a package entry (`@acme/query-utils` →
`src/index.ts` → `export { queryKey } from './queryKey'`) carries its
`Array<string>` return as a nominal reference to the library `Array` interface,
where the same annotation written locally lowers to `T[]`. Property reads on it
were fine — `key.length` resolves through the interface's members — but a
literal index was not: the reference peels to the interface's member object,
which declares no numeric index signature, and the literal-key arm reported the
*receiver* as a missing property (`Property 'key' does not exist on type
'Array<string>'`). tanstack-query's `dehydrated.queries.find((q) =>
q.queryKey[0] === key[0])` was two of the aggregate's false positives.

A reference to `Array<T>` / `ReadonlyArray<T>` with one argument now indexes as
`T[]` before the dispatch, the same test the `Array.isArray` guard already uses
to recognise the generic spelling.
