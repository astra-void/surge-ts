# assignment-target-resolution

tsc resolves the target of an assignment through `checkIdentifier` like any
other read of the name, so it finds whatever a read there finds. surge's
assignment check looked the target up in the enclosing table alone, missing
the module-scope fallback a read consults for a binding declared later — a
`var` written from its own initializer (`var as1 = (as1 = 2)`) and a function
body assigning a module binding declared after it were both reported as
missing (TS2304).

The target still carries its declared type, so the two `number` writes into
a `string` are TS2322, and a name nothing declares is still TS2304; all three
are `tsc` errors too.
