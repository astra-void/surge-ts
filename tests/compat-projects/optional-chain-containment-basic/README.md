# optional-chain-containment-basic

tsc's `narrowTypeByOptionalChainContainment`: what an optional chain starts
from is not nullish wherever the chain is known not to have short-circuited —
it equals a value whose type never includes `undefined` (`null` too, for
`==`), differs from one that always is, has a `typeof` other than
`"undefined"`, or is an `instanceof` something. surge knew this only for
`o?.p === <literal>`, so a call, an element access or a compared variable left
the receiver possibly `undefined` (false TS18048/TS18047). Holds in an `if`, a
conditional, an `&&` chain and at module scope.
