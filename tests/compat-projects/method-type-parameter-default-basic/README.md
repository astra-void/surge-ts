# method-type-parameter-default-basic

A method reached through a property access loses its own type parameters: surge
builds the `FunctionType` without call context, so each one had to be bound to
something. A parameter with a declared default was bound to that default, which
turned an *uninferred* generic into a concrete type — `identity<T = string>(value: T)`
then rejected `defaulted.identity(42)`, and tanstack-query's
`fetchQuery<…, TQueryKey extends QueryKey = QueryKey>` rejected a caller's own
`EnsureQueryDataOptions<…, TQueryKey>`. A default now applies only where no
argument mentions the parameter (`make<T = string>(): T[]`), which is also where
tsc uses it.

Binding the uninferred sentinel instead exposed the second half: `WithRequired<T, K>`
expands to `T & { [_ in K]: {} }`, and when `T` stays generic surge cannot enumerate
that operand, so it keeps the merged surface open with a checker-injected string
index. That marker is not a declared `[key: string]: T`, but assignability read it
as one and rejected an array against the resulting `{}`.

The final assignment is the negative case: tsc rejects it, and so must surge —
neither fix may turn a genuine property mismatch into silence.
