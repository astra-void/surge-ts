# strict-null-checks-off-basic

Without `strictNullChecks`, `null` and `undefined` are in the domain of every
type: they drop out of unions (`number | undefined` is `number`), optional
members and parameters add nothing, they are assignable anywhere, and a
declaration initialized with one widens to `any`. `unknown` is then an
ordinary type that property access, calls, `new` and iteration reject in their
own terms rather than through `checkNonNullType`.
