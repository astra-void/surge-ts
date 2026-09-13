# filter-arrow-predicate-self-reference-basic

`values.filter((c): c is Exclude<typeof c, undefined> => …)` reported a false
`TS2304: Cannot find name 'c'`. The call reads the inline arrow's predicate
target to narrow the filtered element type, and that read resolved the written
type with neither the arrow's parameters in scope nor its diagnostics dropped.
It is a *query* the call makes; the arrow checks its own annotation at its own
span, so anything this resolution reports is a second diagnostic — and one made
from a scope the annotation was never written against.

Only the reporting is dropped. A target the query cannot resolve still yields no
narrowing, which is why `compact` is typed `unknown` here rather than asserting a
narrowed element type.

`namedTargetStillNarrows` is the control: an ordinary predicate arrow still
narrows the result to `string[]`. `theNarrowingIsStillChecked` is the intentional
error and pins that the narrowing is real — the same call assigned to `number[]`
reports.
