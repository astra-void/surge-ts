# import-binding-assignment-basic

An import binding is read-only in the importing module, whatever the export was
declared as, so assigning to one is TS2632 — named, default and namespace
imports alike, at module scope or inside a function. surge treated import
bindings as ordinary constants and reported TS2588. A function-local binding
that shadows the import is an ordinary variable and is pinned as clean.

A member of a namespace import is a read-only property of the module namespace
object, so writing one is TS2540. A member of an ordinary imported object
(`settings.verbose`) stays writable.
