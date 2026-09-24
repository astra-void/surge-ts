# var-case-narrowing-scope

A `case` clause of `switch (v)` narrows `v` for that clause only; the next
clause starts from the type before the switch, and an assignment there is
checked against `v`'s declaration. surge let the narrowing of a `var` escape
the clause's block (a `var` *declared* in a block does stay visible after
it), so the `default` clause checked `v = …` against the case literal. A write
the declaration rejects is still TS2322.
