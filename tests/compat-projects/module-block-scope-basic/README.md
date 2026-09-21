# module-block-scope-basic

A block at module scope (`if (…) { … }`, a bare `{ … }`) is a scope of its
own. surge checked its statements directly in the module's root frame, where
the ambient globals are folded in flat, so a block-local `const name`,
`let status` or `const top` — or a block-local name shadowing a module
binding — was a false TS2451 "Cannot redeclare block-scoped variable". The
shadow types as itself inside the block and the module binding is untouched
after it; an assignment the block makes to a module `let` still narrows it.
