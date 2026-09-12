# annotated-object-method-parameter-basic

A `const x: T = { m(p) { … } }` inside a function body reported the method
shorthand's parameter as an implicit `any` (TS7006), while the same declaration
at module scope and the arrow-property form `{ m: (p) => … }` were fine.

The declaration path re-inferred every annotated initializer *without* the
annotation as its contextual type, to narrow a declared union by what it was
initialized with (`let client: PC | undefined = persisted` starts out as `PC`).
That probe's own diagnostics leaked. It now runs only for a declared union and
discards what it reports; the initializer was already checked against the
annotation. tRPC's `observable()` and `procedureBuilder` are this shape.
