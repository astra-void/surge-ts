# grammar-rest-trailing-comma-basic

tsc's `checkGrammarForDisallowedTrailingComma` with TS1013: a comma after a
rest element or rest parameter is a grammar error, reported at the comma —
in a binding pattern (`checkGrammarBindingElement`), an assignment pattern
(`checkDestructuringAssignment`) and a parameter list
(`checkGrammarParameterList`, which skips an ambient signature). A trailing
comma after an ordinary element or parameter stays legal.
