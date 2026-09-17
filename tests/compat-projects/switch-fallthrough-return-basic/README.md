# switch-fallthrough-return-basic

An empty `case` clause falls through to the next, so an exhaustive `switch`
whose non-empty clauses all return guarantees a return. surge's flow summary
counted the empty clause as not returning and reported a false TS2366; a
switch that really misses a member still reports it.
