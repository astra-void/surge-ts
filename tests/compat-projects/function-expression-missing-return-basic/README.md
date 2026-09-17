# function-expression-missing-return-basic

tsc's `checkAllCodePathsInNonVoidFunctionReturnOrThrow` applies to every
function-like with a written return type: an object-literal getter or
method, a `function` expression and an arrow with a block body report
TS2355, TS2366 or TS2534 just as a declaration does. surge checked only
declarations and class members.
