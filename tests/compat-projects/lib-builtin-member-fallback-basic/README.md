# lib-builtin-member-fallback-basic

surge answers members of arrays, strings, numbers and functions from its own
tables. A member those tables do not list but the configured lib declares on
the global interface behind the receiver (`copyWithin`, `anchor`,
`Function.prototype.caller`) was a false TS2339. It is now read from the lib,
with `T` bound to the element type, so the answer follows `target`/`lib`:
under ES2015 `copyWithin` exists and `at` (ES2022) does not.
