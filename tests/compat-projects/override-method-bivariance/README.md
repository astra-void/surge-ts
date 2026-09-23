# override-method-bivariance

tsc compares a method's parameters bivariantly even under
strictFunctionTypes (`compareSignaturesRelated` for a method declaration),
so a method may override one taking a wider parameter (`handle`, `visit`).
A property holding a function type is compared strictly, so narrowing
`callback`'s parameter is TS2416.
