# discriminated-literal-report-basic

`getMatchingUnionConstituentForObjectLiteral`: an object literal that
writes a discriminant belongs to the constituent admitting that value, so
an excess key is reported there (TS2353) and a literal that satisfies no
constituent is reported against the union itself. surge tied on property names and reported the literal against whichever
member came first, naming a bad discriminant instead of the real error.
The report only ever replaces one surge already made: a literal it
accepts is one tsc accepts too, and a contextually typed callback's
returned literal is checked without freshness.
