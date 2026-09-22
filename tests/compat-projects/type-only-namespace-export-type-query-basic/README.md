# type-only-namespace-export-type-query-basic

`import type { Utils }` of an `export * as Utils` namespace binds the whole
symbol, value side included: tsc's `resolveName` only rejects a type-only alias
in a value position outside a type query (`IsValidTypeOnlyAliasUseSite`), so
`typeof Utils.RuleCreator` resolves. surge bound only the namespace's qualified
types when the module also exported a type, so the query reported TS2304 and
hid the assignment error behind it. A value use of `Utils` is still TS1361.
