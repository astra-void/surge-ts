# super-type-arguments-follower

tsc's `parseSuperExpression` takes type arguments after `super` when
`parseTypeArgumentsInExpression` accepts them (a template or `(` may follow,
and so may anything that cannot start an expression, `.` included), reports
them as TS2754, and then requires `(`, `.` or `[`. A tagged template after
`super<number>` is therefore TS1034 on the template, and as a parse error it
leaves the program's semantic diagnostics unreported.
