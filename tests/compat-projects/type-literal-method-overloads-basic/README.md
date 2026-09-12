# type-literal-method-overloads-basic

Same-named function members of an **interface** body were folded into one
permissive signature; the same members written in a **type literal** were not.
`resolve_object_type` inserted each property into the map in source order, so
the last declaration won and every earlier overload was dropped. A call that
matched an earlier overload was then checked against the last one's arity and
reported `TS2554`.

ts-pattern writes its whole builder as a type alias, and `Match<i, o>` declares
four `.with` overloads whose last is a three-parameter form. Every
`match(x).with(pattern, handler)` in the corpus — 431 diagnostics, 37 files —
was one instance of this. `Chain<i>` here is that shape reduced to what
matters: a type alias, three `with` overloads, the two-parameter one first.

The type literal now collapses a repeated function member the way the interface
path does, through `merge_overload_signatures`. `interfaceStillGroups` is the
control: the interface spelling of the same pair already worked and must keep
working.

`assigningTheMergedReturnToNumberReports` is the intentional error and pins the half of the
merge that arity alone would not: the collapsed signature keeps `string` as its
return, so assigning it to `number` still reports. Only the non-generic
spelling pins it — when the overloads are generic the merged return degrades to
the `Unknown` sentinel, for the interface and the type literal alike, so a
generic chain's result carries no error to pin. That degradation is older than
this fixture and is not what it is about.

Wrong-arity rejection is deliberately not pinned here. `tsc` phrases an
overloaded call's arity error as a *range* ("Expected 1-2 arguments"), which a
single collapsed signature cannot produce, so pinning it would add message drift
at a location that is otherwise correct.
