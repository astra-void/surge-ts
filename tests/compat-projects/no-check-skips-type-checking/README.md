# no-check-skips-type-checking

`noCheck` makes tsc skip type checking for every file (`SkipTypeChecking`):
the mismatched initializer (TS2322), the unresolved name (TS2304) and the
implicitly-`any` parameter (TS7006) are all left unreported, and only
syntactic diagnostics would remain.
