# arguments-in-initializer-basic

`checkIdentifier`'s TS2815: `arguments` read from a property initializer or
static block when it still resolves to an enclosing function's `arguments`
(an arrow in between does not shield it; a `function` expression does).
