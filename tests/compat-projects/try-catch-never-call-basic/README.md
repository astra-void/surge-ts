# try-catch-never-call-basic

A `catch` that ends in a call typed `never` (`process.exit(1)`, a `die()`
helper) does not fall through, so the code after the `try` is reached only
from a block that completed and what the block assigned is definitely
assigned. surge's try/catch join consulted only the syntactic flow summary
of the handler, which cannot know a callee's type, so `let doc; try { doc =
await g(); } catch { process.exit(1); } doc.a` was a false TS2454. The `if`
form already used the typed helper; the try form now does too. A handler
that merely logs, or exits only conditionally, still leaves the variable
possibly unassigned.
