# script-expando-const-seed

A script's top-level values are seeded into the global table while function
signatures are collected (so `function f(x: typeof g.x)` reads them), then
removed so the check phase declares each one itself. A `const` holding a
function with expando members (`const f = function () {}; f.x = 1`) had its
seed replaced by the expando-merged value during collection, which the removal
did not recognise; the leftover made the `const` its own redeclaration (a false
TS2451). A genuine redeclaration is still TS2451 at both declarations.
