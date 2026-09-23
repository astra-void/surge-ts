# script-global-shadowed-by-module-local

tsc merges every script file's top-level declarations into the one global
table (`initializeChecker`), and a module's name lookup — at its top level and
inside its functions — reaches it once the module's own scope misses. A
module-local declaration of the same name (`local.ts`'s `v2`, `reader.ts`'s
`v4` and `shared`) shadows the global only inside that module: the module's
scope is a declaration space of its own, so its `let shared` is no
redeclaration of the script's. surge gave modules no view of script globals
at all, so every such read was a false TS2304. `wrong` and `wrongInBody` read
the global `v2: number`, which are the two intentional errors.
