# write-targets-checked

What tsc checks around a write it rejects or a write oxc does not lower to a
name: the target and value of an assignment to an invalid target, a
`for…in`/`for…of` head that is a member, a pattern or an invalid target (its
loop body is still checked), and a `let` declaration after a label or as an
`if` body, which tsc parses as a declaration and rejects from its checker.
