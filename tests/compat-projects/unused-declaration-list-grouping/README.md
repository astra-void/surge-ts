# unused-declaration-list-grouping

With `noUnusedLocals`, tsc groups unused bindings by what declared them
(`reportUnusedVariables`): a declaration list of several declarations none of
which is read is TS6199 on the list, a destructuring pattern of several elements
none of which is read is TS6198 on the pattern — counting a nested pattern as
unread only when all of its names are — and anything else reports each unread
name (TS6133). An `_`-prefixed array element or renamed object element, and an
object element before a rest element, count as read; a shorthand `_` name does
not.
