# namespace-merged-function-export-basic

A `namespace` merged into a same-named value contributes its value members to
that value. surge builds that merge into the exportable-value table, but the
export collector read a *function* export straight out of the local symbol
table, which holds the bare function — so `paths.realpathSync.native` was a
missing property. tRPC's OpenAPI generator calls `fs.realpathSync.native`,
declared exactly this way in `@types/node`.

The merged shape — an object that kept the call signature — now wins for
function exports too. The namespace's members are still published permissively,
so this fixture pins that they *resolve* and stay callable, not what they
return; `stillChecked` pins that the function's own signature survives the
merge.
