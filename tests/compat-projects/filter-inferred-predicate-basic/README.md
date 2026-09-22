# filter-inferred-predicate-basic

tsc infers a type predicate from an unannotated single-parameter arrow whose
body is one boolean expression (`getTypePredicateFromBody`): the parameter's
true-branch narrowing is the predicate when it cannot survive the false branch.
`events.filter((event) => event.type === 'state')` is then an array of the
`state` variant; the `&&` form keeps `Event`, since `state: 'idle'` passes the
false branch, and reading `error` off it is TS2339.

A function is assignable to an object whose index signature is `any` (tsc skips
the members for an `any` indexer), but not to one indexed by `unknown`.
