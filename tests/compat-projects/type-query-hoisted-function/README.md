# type-query-hoisted-function

tsc's binder declares every function of a scope before any signature is
resolved, so a type alias or interface whose body queries a function
(`type Later = ReturnType<typeof later>`) reads it wherever a signature names
that type — even a signature declared above the function. surge hoisted a
scope's functions for signature collection only when a signature queried one
directly, so reaching it through a local type was a false TS2304; and its
hoisting pass collected signatures in source order, so a type resolved on the
way read the sentinel of a function not collected yet and was memoized that
way. The five intentional errors each read a queried function's return type,
from a signature, from a top-level annotation, and inside a function body.
