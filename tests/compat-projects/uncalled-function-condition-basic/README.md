# uncalled-function-condition-basic

TS2774: an `if` or conditional-expression test that tests a function for
truthiness is always true, and tsc reports it (`checkTestingKnownTruthyType`)
unless the function is used where the test holds — mentioned in the then branch
or `whenTrue` operand, or later in the same `&&` chain. Inside a conditional
expression's test, an `&&` operand sees only its chain, not `whenTrue`. The tested positions are the condition itself, the right operand of a
top-level logical expression and each operand along its `||`/`??` chain, and
the left operand of every `&&`.

Whether the function is used again is answered from the source by the parser,
by name for a binding and by member path for a member (tsc compares symbols);
the checker reports when the tested type is certainly callable. A possibly
undefined function (`F | undefined`, an optional method) and a negated test are
pinned as clean. An `&&` outside any condition (`const x = fn && 1`) is not covered yet.
