# unresolved-import-callback-implicit-any-basic

A binding whose module was reported unresolved is tsc's error type, so a
callback passed to a call on it genuinely has no contextual parameter type and
tsc reports TS7006 on its parameters. surge suppresses implicit-any wherever it
lost the context itself — the right default — but that suppression is a depth
counter, so an *enclosing* degraded expectation hid these too. The `return`
position is exactly such an enclosing frame: an unannotated function's return
carries the sentinel as its expected type.

The provenance verdict now outranks the ambient guess: when the receiver's `any`
is the source's, the argument walk clears the ambient depth rather than adding
to it. Everything else about the suppression is unchanged — a chain that merely
*collapsed* to `any` partway still suppresses, because tsc contextually types
those callbacks and reporting there would describe surge's gap.
