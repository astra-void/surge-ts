# dependent-destructuring-own-guards

A destructured binding narrowed by a sibling's discriminant starts from the
read off the surviving union members, which its own earlier guards then narrow
(tsc's `getNarrowedTypeOfSymbol` feeding `getFlowTypeOfReference`): inside
`if (payload)`, `kind === "A"` leaves `payload` a `number`. An equality with
`null` discriminates like a literal. Reads that stay possibly `undefined` or
use a missing member are still reported.
