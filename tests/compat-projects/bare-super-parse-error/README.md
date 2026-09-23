# bare-super-parse-error

`super` must be followed by an argument list or a member access; anything else
is a parse error (TS1034, tsc's `parseSuperExpression`) reported on the token
after the keyword, which oxc accepts. Like every parse error it makes the
program report its syntactic diagnostics alone, so the misplaced `super`s and
the mismatched initializer are not reported.
