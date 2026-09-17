# interface-extends-member-compatibility-basic

An interface declaration was not checked at all in the program pass — only
collected — so TS2430 had no emission point.

Unlike the class checks this has no member-specific pass. Extending an
interface can only fail by overriding an inherited member incompatibly, so that
single condition is the whole rule, and tsc reports it **once per offending
clause, on the interface name**, however many members conflict: `Multi`
mistypes two members and still gets one diagnostic. `BadOfTwo` pins that the
clause named is the one whose member actually conflicts (`A`, not `B`).

Narrowing an inherited property (`p: 5` against `p: number`) is legal and is
pinned as such. A generic interface and a clause with type arguments are
skipped for the same reason the class checks skip them.
