# typeof-guard-unreachable-branch-basic

`typeof x === 'string'` over a union with no string member left the whole union
standing. Nothing survives the filter there, which the guard read as "no
narrowing" — but it is the stronger fact: every member carried a tag (an
untagged member is kept by the filter), none of them lands in this branch, so
the branch is unreachable and tsc types the subject `never`.

`fromEitherShape` is drizzle's shape and the seven diagnostics this closed
there: `typeof connection === 'string' ? createClient({ url: connection }) : …`
over a `Config | undefined` reported the union against `url`'s `string`, where
tsc checks `never` and says nothing. `theStringBranchIsUnreachable` is the same
fact as a statement, and `theOtherBranchIsUnreachableToo` pins the other
direction — when *every* member matches, the else branch is the empty one.

`aUnionWithAStringStillNarrows` is the control: a union that does have a string
member narrows to it as before.

`theSurvivingMembersAreStillChecked` is the intentional error and pins that the
surviving members keep their own precision — the else branch is
`Config | undefined`, so reading `.url` off it still reports.
