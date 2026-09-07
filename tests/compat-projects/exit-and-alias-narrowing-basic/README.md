# exit-and-alias-narrowing-basic

Two flow facts surge could not see.

A call to a `never`-returning function ends control flow exactly as a `return`
does, so `if (!file) { …; process.exit(1); }` narrows `file` in the
fall-through. The body-flow summary is purely syntactic and cannot read a
return type, so this one case is decided where the scope is in hand — for a
bare call and for a member call alike.

A `const` initialized from a condition stands for that condition: `if (named)`
narrows by `name !== undefined && name.length > 0`, which is what tsc's
aliased-condition narrowing does. A `const` bound to a *property reference* is
the discriminant half of the same feature: testing `direction` narrows the
`message` it was read from, whether it was written as a destructure or as a
plain property read.

Both narrowings apply at once, which `aliasKeepsItsOwnNarrowing` pins: the
written condition still proves the alias binding itself non-nullish, so
rewriting it to the object reference must not replace that.

Both are restricted to `const`, which `reassignedAliasIsNotAGuard` pins: a
`let` that has been written to is not an alias, so `name` there is still
`string | undefined` and the call is the single intentional error.
