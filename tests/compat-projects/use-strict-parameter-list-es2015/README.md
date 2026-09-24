# use-strict-parameter-list-es2015

use-strict-parameter-list-basic under `target: es2015`. tsc's
`checkGrammarForUseStrictSimpleParameterList` runs only from ES2016 on, so a
`"use strict"` prologue in a function with a non-simple parameter list is
neither TS1346 nor TS1347 here.
