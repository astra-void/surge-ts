# string-keyword-indexed-access-basic

`T[string]` — the `string` *keyword* as an index — was rejected outright with
`TS2538`. It reads the receiver's string index signature, which is what makes
`Record<string, V>[string]` mean `V`, and is the normal way to name a `Record`'s
value type. drizzle writes it twice (`SelectedFieldsFlat<TColumn>[string]` and
`UpdateSet[string]`) and both reported.

`fromRecord`, `fromInterface` and `fromNested` are the shapes that were broken —
through an alias, through an interface's own index signature, and chained.
`aLiteralKeyStillReads` is the control for the path that always worked.

`theValueTypeIsTheRecordsOwn` is the intentional error and pins the direction:
the access really does resolve to the record's value type, so reading a property
that type does not have still reports.

Not covered: a receiver with no index signature at all. tsc answers `TS2537`
there, which surge does not have, so that receiver degrades silently rather than
report a code tsc never does — a false negative, deliberately, over a false
positive.
