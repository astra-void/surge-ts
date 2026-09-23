# jsdoc-unknown-type-in-typescript

tsc parses a `?` in type position as a nullable type and then requires the
type (`parseJSDocNullableType`), so `identity<?>` is the parse error TS1110 at
the `>`. That syntax error makes the program report its syntactic
diagnostics alone: the TS17019 of `string?` and the TS2322 below it are not
reported.
