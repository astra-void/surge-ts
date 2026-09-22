# jump-targets-and-labels-basic

tsc's `checkGrammarBreakOrContinueStatement` walks from a `break`/`continue` to
its target: a function or class static block in between is TS1107, a
`continue` whose label is not on a loop is TS1115, and a jump with no target at
all is TS1104/TS1105 (unlabeled) or TS1115/TS1116 (labeled). A label repeated
inside itself is TS1114, and a label on a declaration is the binder's TS1344.
