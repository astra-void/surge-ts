# tsc-parser-statement-recovery

tsc's parser recovers from a syntax error and keeps going, reporting each
error once at the position its own recovery reaches (typescript-go
`internal/parser`). One file per error shape: missing and unexpected tokens,
list recovery (`parsingContextErrors`), invalid names, misplaced
declarations, and the parser's own grammar messages. A program with any syntax
error reports only its syntactic diagnostics.
