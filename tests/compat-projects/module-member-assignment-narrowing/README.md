# module-member-assignment-narrowing

A write to a member narrows it for the code that follows at module scope as
it does in a function body (tsc's flow narrowing by assignment):
`target.run()` and `target.label.toUpperCase()` read the assigned types. A
function declaration is a flow container of its own and sees no module
narrowing — of a member (`target.run()` inside `later` is TS2722) or of a
`const` asserted defined (`maybeText.length` inside `readLater` is TS18048)
— while an arrow sees the `const`'s narrowing.
