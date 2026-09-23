# script-typeof-global-signature-basic

A script's top-level `var`s are globals the binder declares before any
signature is resolved, so `function f(x: typeof a)` reads `a` wherever it is
written — later in the file or in another script. surge collected global
function signatures before any script value existed, so each such `typeof`
was a false TS2304 (or TS2552 with a same-letter class in scope) and the
parameter degraded. An undeclared name still reports.
