# null-type-basic

`null` as a type of its own. surge parsed the `null` keyword as `undefined` and
typed a `null` literal as `any`, so `var a: string = null` was silently accepted
and every `T | null` read as `T | undefined`. Covers assignment (TS2322/TS2345),
`=== null` vs `== null` narrowing, `null` as a discriminant, the possibly-null
receiver matrix (TS18047/TS18049/TS2531/TS18050), possibly-null calls
(TS2721/TS2723), and tsc's display rules: a non-nullable source is reported
against the lone non-nullable member of a `T | null` target, a literal source
keeps its literal type against a `null` target, and an array literal reports
every failing element.
