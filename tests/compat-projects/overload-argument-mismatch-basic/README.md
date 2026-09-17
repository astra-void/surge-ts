# overload-argument-mismatch-basic

tsc resolves an overloaded call against the candidates whose arity fits the
arguments (`hasCorrectArity`). When several fit and none accepts the arguments
the call is TS2769; when exactly one fits, its own argument error is reported
(TS2345 against that candidate's parameter). surge checked every call against
the group's permissive fold, so it reported TS2345 against a union of the
candidates' parameters in both cases.

The fold is still used for evaluation; a mismatch against it implies one against
every fitting candidate at that position. The last line pins that a matching
call still resolves its own overload's return type.
