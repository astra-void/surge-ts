# signature-parameter-typeof-scope-basic

A signature's parameter names — plain, destructured or rest — are in scope
for the parameters after them and for the return type, so `typeof x`,
`typeof alias` (from `{ a: alias }`) and `typeof rest` read the parameter.
surge bound no function-type parameter at all and no destructured name of a
declared function, so each was a false TS2304 and the type degraded.
