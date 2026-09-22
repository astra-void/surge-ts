# call-and-modifier-context-basic

A value that can only be constructed, called without `new` (TS2348 from
`resolveCallExpression`, naming its type — surge reported the generic TS2349);
a write to `undefined`, which is not a variable (TS2539 — surge reported it as
unresolved); an `async` constructor (TS1089); and `export`/`declare` on a
statement that is not a module element (TS1184, tsc's
`checkGrammarModuleElementContext`).
