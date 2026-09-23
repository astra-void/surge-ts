# property-initialization-literal-names-basic

tsc's `checkPropertyInitialization` (checker.go) reports TS2564 only for a
property named by an identifier, a private name or a computed name. A string
or numeric literal key is never checked, however it is spelled.
