# ambient-block-import-signature-basic

An `import` written inside `declare module "x" { … }` is one of the block's
locals (Go's binder declares it in the module's symbol table), so the block's
function signatures resolve names through it wherever the imported block sits
in the program — here in a file that comes later. surge mapped the signatures
before any block import was bound: every parameter typed through one was a
false TS2304, and callers got a degraded parameter they could never fail.

A name the block neither declares nor imports still reports.
