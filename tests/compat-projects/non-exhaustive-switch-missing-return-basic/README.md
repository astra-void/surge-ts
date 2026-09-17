# non-exhaustive-switch-missing-return-basic

A function whose body ends in a `switch` without `default` falls off its end
unless the cases cover the discriminant (`isExhaustiveSwitchStatement`), so a
declared return type that does not admit `undefined` is TS2366. surge's flow
summary treated any such switch whose clauses all return as returning.

Exhaustiveness is decided while the body is checked: a discriminant typed as a
union of literals (a plain binding or a discriminant property) or `boolean` is
covered when every member is named by a case, and `string`/`number` never are.
Exhaustive switches, a `default`, and a return type admitting `undefined` are
pinned as clean. An `enum` discriminant is not decided yet.
