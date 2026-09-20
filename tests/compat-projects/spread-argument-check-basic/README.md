# spread-argument-check-basic

`getEffectiveCallArguments`: a tuple spread is one argument per element and
an array spread is one argument of its element type, each related to the
parameter at its effective position. `hasCorrectArity` reports TS2556 for an
array spread that begins before the required parameters are supplied or past
the last parameter of a signature without a rest. Arguments a non-array rest
type receives are left to the gathered-tuple relation. An immediately invoked
function takes its parameter types from the same effective arguments. surge
skipped every spread argument.
