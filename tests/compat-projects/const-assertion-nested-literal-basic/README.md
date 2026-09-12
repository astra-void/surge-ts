# const-assertion-nested-literal-basic

`as const` was not applied at all on the inference path. The sketch that infers a
call's type arguments forwarded straight through the assertion to the inner
expression, so `[[1, 2, 3]] as const` reached inference as `number[][]`: the
outer array never became a tuple and neither did the inner one. Inferring `B`
from `readonly B[]` against that produced `number[]` — or, through a callback
parameter, nothing at all and `any`.

ts-pattern builds its exhaustiveness cases exactly this way,
`flatMap(range, (x) => [[x, x, x]] as const)`, and the tuple that comes out is
then passed to a function wanting `[1 | 2 | 3, 1 | 2 | 3, 1 | 2 | 3]`.

The assertion is now honoured recursively on that path: an array literal is a
tuple, a nested one a nested tuple, an object literal an object, and leaves go
through ordinary inference so their literal types survive. The declaration and
expected-type paths already did this; only the inference sketch did not, which
is why the same `as const` behaved differently as an argument than as an
initializer.

The fixture is 0/0 by design: without the fix it reports three, and the shapes it
pins are the three routes into inference — a direct argument, a callback return,
and an object literal nested inside the asserted array.
