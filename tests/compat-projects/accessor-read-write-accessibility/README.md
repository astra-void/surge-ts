# accessor-read-write-accessibility

An accessor pair can give its getter and setter different accessibility.
tsc checks a read against the getter and any assignment target — a plain
write, a compound one (`+=`) or a `this.x = v` in a method — against the
setter alone (`getDeclarationModifierFlagsFromSymbol` with `isWrite`), so a
compound write through a public setter is fine even when the getter is
private.
