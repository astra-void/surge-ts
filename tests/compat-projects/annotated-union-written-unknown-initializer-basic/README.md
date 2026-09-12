# annotated-union-written-unknown-initializer-basic

A declared union narrows to what its initializer can inhabit, but the probe that
re-infers the initializer discarded any type carrying an `unknown` — the walker
it asked treats surge's degradation sentinel and a *written* `unknown` alike.
zod's `JSONSchema` is `{ [k: string]: unknown; … }`, so
`const defs: JSONSchema["$defs"] = ctx.external?.defs ?? {}` kept its `|
undefined` and every use of it was a false error.

The probe now asks only whether the sentinel is present. `collectUnnarrowed`
pins the other direction: a parameter that really can be `undefined` still
reports.
