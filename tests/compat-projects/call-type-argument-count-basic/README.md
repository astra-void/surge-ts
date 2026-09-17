# call-type-argument-count-basic

tsc's `resolveCall` checks written type arguments: a count outside the
signature's range is TS2558, type arguments on an untyped (`any`) callee are
TS2347, and a type argument violating its parameter's constraint is TS2344.
surge checked none of these. The constraint is related for the first type
argument only, the one whose position is known.
`new C<…>()` is checked the same way against a class declared in a source
file.
