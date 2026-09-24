# assignment-contextual-callback

tsc contextually types an assignment's value by its target
(`getContextualTypeForAssignmentExpression`), inside a function body as at
the top level: a callback assigned to a typed parameter or local — with `=`,
`&&=` or `||=` — has typed parameters. An auto-typed `let` or an `any`
target gives no contextual signature, so those callbacks' parameters are
implicitly `any` (TS7006).
