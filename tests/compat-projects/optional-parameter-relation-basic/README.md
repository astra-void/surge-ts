# optional-parameter-relation-basic

`getTypeOfParameter`: an optional parameter is `T | undefined` on both
sides of a signature comparison, so `(x: number) => number` does not fit
`(x?: number) => number` — the target may omit the argument. The same
type is what an unannotated callback parameter takes from an optional
contextual one, minus `undefined` again when it has a default. An enum's
members are read-only, and `typeof undefined` names the `undefined` type.
