# export-assignment-type-only-alias

tsc's `IsValidTypeOnlyAliasUseSite`: the bare identifier of `export =` or
`export default` is not an expression node, so naming a type-only alias there
(`import { A }` of an `export type { A }`) is not a use of its value — no
TS1361/TS1362. Parenthesized (`export default (A)`) or as the head of a member
access (`export = A.s`), it is one, and TS1362 stands.
