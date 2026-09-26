# logical-truthiness-and-literal-checks

tsgo's `checkTestingKnownTruthyTypes` on the left operand of every `&&`
(expression statements, initializers, returns, conditional tests) and on every
operand of an `if` condition's logical chain; regular expression literals typed
as the global `RegExp`; destructured private and protected members; literal
freshness of returns read from parameters; object spread of a union with several
object members distributing into a union.
