# lookup-error-type-propagation-basic

A member the receiver lacks is reported once (TS2339) and then typed with
TypeScript Go's `errorType`, which is `any`: a call on it goes through
`resolveErrorCall`, a callback returning it binds the naked `$Output` to `any`
(`inferFromTypes`), and `any` absorbs a union (`getUnionType`,
`checker.go:26006`). Every callback later passed to a call on that `any` has no
contextual signature, so its parameters are implicit `any` (TS7006).

surge typed the failed lookup with its own "could not model" sentinel, which
suppresses implicit-any, so the whole chain downstream of one missing member
went silent. This is the tRPC example shape: a resolver reads a model the
generated Prisma client does not have, and every `query.data.map((row) => …)`
on the client is a TS7006 in tsc.

The last statement pins that the instantiation itself still renders `any` in
the position the error type reached.
