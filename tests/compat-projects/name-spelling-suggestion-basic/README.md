# name-spelling-suggestion-basic

tsc's `resolveNameHelper` offers the closest visible name as TS2552 for an
unresolved type reference as well as a value — declared types, lib types and
in-scope type parameters — and `undefined` is a global value candidate. surge
suggested value bindings only, so these were TS2304.
