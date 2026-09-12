# argument-mismatch-first-only-basic

tsc checks a call's arguments in order and reports the first one that is not
assignable, then stops — a call with two bad arguments carries one TS2345, for
a plain, tuple-rest, array-rest, or callable-object signature alike. surge
reported every mismatching argument.

Later arguments are still evaluated: a call nested inside one keeps its own
first mismatch (`pair(1, nested(2, 'b'))` reports twice, once per call), and
their diagnostics and contextual typing are unchanged. Only the mismatch verdict
against the enclosing call's parameter is withheld.
