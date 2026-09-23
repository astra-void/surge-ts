# type-query-hoisted-function

tsc's binder declares every function of a scope before any signature is
resolved, so a type alias or interface whose body queries a function
(`type Later = ReturnType<typeof later>`) reads it wherever a signature names
that type — even a signature declared above the function. surge hoisted a
scope's functions for signature collection only when a signature queried one
directly, so reaching it through a local type was a false TS2304. The two
intentional errors read the queried functions' return types at the top level.
