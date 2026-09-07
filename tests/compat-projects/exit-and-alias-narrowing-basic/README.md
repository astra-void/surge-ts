# exit-and-alias-narrowing-basic

Two flow facts surge could not see.

A call to a `never`-returning function ends control flow exactly as a `return`
does, so `if (!file) { …; process.exit(1); }` narrows `file` in the
fall-through. The body-flow summary is purely syntactic and cannot read a
return type, so this one case is decided where the scope is in hand — for a
bare call and for a member call alike.

A `const` initialized from a condition stands for that condition: `if (named)`
narrows by `name !== undefined && name.length > 0`, which is what tsc's
aliased-condition narrowing does. Only `const` bindings with a plainly boolean
initializer are recorded, which is what the last function pins: a reassigned
`let` is not a guard, so `name` there is still `string | undefined` and the
call is the single intentional error.
