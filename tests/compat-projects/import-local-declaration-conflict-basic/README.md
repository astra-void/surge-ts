# import-local-declaration-conflict-basic

tsc's `checkAliasSymbol`: an import whose local name is also declared in the
file is TS2440 on the import — but only when the two overlap in meaning, so a
value import beside a local `type` alias (and a type import beside a local
`const`) is legal. surge reported nothing for the type collisions and TS2451
on the local declaration for the value ones.
