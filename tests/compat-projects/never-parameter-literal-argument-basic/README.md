# never-parameter-literal-argument-basic

surge never relates an argument to a `never` parameter, since such a call is
usually an exhaustiveness assertion and its narrowing under-approximates
tsc's. An argument written as a literal is not narrowed by anything, so it is
now reported (TS2345) — including the `never` a union of incompatible
callables intersects its parameter to.
