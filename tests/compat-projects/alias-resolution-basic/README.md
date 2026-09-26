# alias-resolution-basic

tsc resolves an alias to what it names through imports, re-exports and
`export =` (`resolveAlias`). An alias whose resolution comes back to itself is
TS2303 at every alias along the cycle; aliases that only lead into a cycle
report nothing. The global merge resolves aliases too: an
`export as namespace` name conflicts with a `declare global` value of that
name as the module it names does, and a module augmentation's declaration
conflicts with what a re-exported name resolves to.
