# declaration-form-grammar-basic

Declaration-shape grammar from tsc's `checkGrammarVariableDeclaration`,
`checkGrammarProperty`, `checkGrammarObjectLiteralExpression` and
`checkGrammarForDisallowedBlockScopedVariableStatement`: a definite `!` with an
initializer (TS1263), without a type (TS1264), or in an ambient declaration
(TS1255); two accessors of one kind in an object literal (TS1118, after which
tsc stops checking that literal, plus the binder's TS2300 on both); and a
`const` as the whole body of an `if` or loop (TS1156).
