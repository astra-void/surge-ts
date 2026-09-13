# switch-case-comparability-basic

A `case` test was not checked at all: neither what is written in it (the
unresolved name in `byString`) nor whether its type can equal the
discriminant's (`TS2678`), which is the same relation `===` uses for `TS2367`
and reports through the same widened-operand naming.

`byEnum`, `byLocal` and the `'a'` case pin the non-reports: an enum member, a
local `const`, and a case that really is one of the union's members.
