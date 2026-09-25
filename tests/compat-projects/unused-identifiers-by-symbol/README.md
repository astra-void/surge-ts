# unused-identifiers-by-symbol

Under noUnusedLocals and noUnusedParameters tsc reports a declaration no use
resolves to (`checkUnusedIdentifiers`): a use marks the symbol its name
resolves to, so a local shadowed by an inner one, a function or class that
only names itself, or a variable only ever written counts as unused. It
checks every scope — namespaces, blocks, `switch` cases, loop heads, class
methods — and a module's own classes, interfaces, enums and type aliases
(TS6196). A private or `#` member is used only when read through the class:
`this.x`, `Class.x`, a literal key or one whose type is a union of literals,
not a write alone and not a method calling itself; a private parameter
property the constructor only reads as a parameter is TS6138. A type
parameter list whose every parameter is unused reports once (TS6205), and a
mapped type's key is never reported.
