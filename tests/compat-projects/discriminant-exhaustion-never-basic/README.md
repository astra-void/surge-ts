# discriminant-exhaustion-never-basic

tsc's `narrowTypeByDiscriminant` and `narrowTypeByEquality` filter the
declared type, and `filterType` yields `never` once nothing is left. That is
what makes the exhaustiveness idiom — `const unreachable: never = shape` in the
`default` of a `switch`, or after the last `if … return` — type-check. surge's
discriminant narrower only filtered *unions* and gave up when the filter
emptied, so the last member survived and the idiom was a false TS2322; the
composed (`||`, `switch`) literal-equality leaf had the same gap for a plain
literal union. `never` is produced only when the member's *required*
discriminant is exactly the excluded literal: a literal-union discriminant
and a switch that skips a member both keep it.
