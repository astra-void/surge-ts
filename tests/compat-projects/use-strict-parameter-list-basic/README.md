# use-strict-parameter-list-basic

`checkGrammarForUseStrictSimpleParameterList`: a `"use strict"` prologue in a
function whose parameter list has an initializer, a binding pattern or a rest
parameter — TS1346 on each such parameter and TS1347 on the directive. A
directive that is not in the prologue, or an expression-bodied arrow, does
not count.
