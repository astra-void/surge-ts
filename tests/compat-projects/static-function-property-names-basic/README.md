# static-function-property-names-basic

Without `useDefineForClassFields` (off below target ES2022), a static member
named like one of `Function`'s own properties collides with the constructor
function's — TS2699, tsc's `checkClassForStaticPropertyNameConflicts`. An
anonymous class expression is named by the variable it initializes.
