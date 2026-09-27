# global-declaration-ignores-consumer-locals

A global declaration resolves under the global scope, whichever module's code
reached it. Here the module declares its own `IteratorYieldResult`. The lib's
`Iterator.next(): IteratorResult<T, TReturn>` still names the lib's
`IteratorYieldResult`: `result.value` is the `number` it yields (the `string`
binding reports), and the module-local `bogus` member is not on it (TS2339).

Resolving the lib alias under the reading module's scope (or reading the
module's local table from a context recovered for it) made `value` missing and
`bogus` present — one false positive and one false negative.
