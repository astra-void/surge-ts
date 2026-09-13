# overload-group-generic-parameter-fold-basic

An overload group's value type is the permissive **fold** of every overload: a
position declared differently across them becomes the union of what they accept.
A *generic* group never saw that fold. Instantiating a generic call rebuilds
every parameter from the one parsed signature the group keeps — the first
declaration's — which throws the union away and leaves only the first overload's
shape. An argument written for a later overload was then reported against the
first: `fromTheSecondOverload` is the whole class, reported as missing
`initialData`.

Each later overload is now instantiated against the same call and its parameter
unioned back in. Diagnostics raised while resolving an overload the call did not
pick are discarded, since a constraint violation or an unresolved name in an
unrelated overload is not the call's error.

Only the parameters are folded here. The kept signature's **return type**
survives the fold: widening it to the group's union, or to `any` the way the
non-generic fold does, would degrade every generic group's result. Which
overload's return a call gets is decided afterwards, from the evaluated
arguments — see `overload-return-selection-basic` — so `fromTheSecondOverload`
now has `{ data: TData | undefined }` and not the first overload's
`{ data: TData }`.

`withExplicitTypeArguments` covers the other instantiation path: type arguments
supplied at the call rather than inferred from it.

This project is deliberately clean under the oracle, so the control for the
opposite direction lives in `crates/surge-ts-checker/tests/function_overloads.rs`
(`a_property_matching_no_generic_overload_still_reports`) instead: a property
matching neither overload must still report, and it does. It cannot live here,
because tsc answers every failing overloaded call with `TS2769` and surge — with
no real overload resolution — never emits that code, so the preset could not be
green with a failing call in it.

tanstack-query's `useQuery({ queryKey, queryFn })` is the shape this came from:
its first overload demands `initialData`, and six of that corpus's false
positives were this one cause.
