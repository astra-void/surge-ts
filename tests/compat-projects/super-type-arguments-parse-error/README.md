# super-type-arguments-parse-error

Type arguments on `super` are a parse error (TS2754, tsc's
`parseSuperExpression`), which oxc accepts. Like every parse error it makes the
program report its syntactic diagnostics alone, so the misplaced `super()` and
the mismatched initializer are not reported.
