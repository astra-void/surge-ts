# nested-predicate-enclosing-type-parameter-basic

A type-predicate function declared inside a generic function, whose target is
the *enclosing* function's type parameter (`function isTargetFunction(node:
string): node is TFunc` inside `createOrderRule<TFunc, …>`), used as an `if`
guard from within a callback.

Narrowing re-resolves the predicate's written target at the guard site, and
that resolution passed only the guard's own type-argument substitution — which
holds the *predicate's* type parameters, and this predicate has none. The
enclosing generic's parameters live on the checker's type-parameter scope
stack, which the narrowing path never consulted, so `TFunc` resolved as an
unresolved name and TS2304 was reported against the declaration's span, long
after the declaration itself had checked clean.

The resolution now merges the active scopes the way every other annotation
resolution does (`merged_type_parameter_substitution`), so the enclosing
parameter is seen. It resolves to a placeholder rather than a concrete type,
so the guard still declines to narrow — the fix removes a false positive, it
does not add narrowing.
