# union-alias-callback-return-inference-basic

A callback returning `string` against `() => MaybePromise<O>` binds `O` to
`string`. TypeScript Go's `inferFromTypes` (`internal/checker/inference.go`)
sees only the alias's resolved union `Promise<O> | O` and goes through
`inferToMultipleTypes`, which hands the unmatched source to the naked `O`.
surge entered a union alias body only when the argument was itself a union, so a
primitive source inferred nothing and the call's result went silent. The same
call spelled with the inline union already worked.

The negatives pin that a `Promise` result still infers the awaited type and a
matching result is accepted.
