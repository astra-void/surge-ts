# unresolved-import-type-references

An import whose module does not resolve binds tsc's `unknownSymbol`, whose
every meaning is the error type: `Missing.Type`, `Namespace.Type` and a
default import used as a type are the error type, reported nowhere, and a
callback they type is an implicit `any` (TS7006). A class deriving from one
has no base type (`resolveBaseTypesOfClass`), so what it would inherit is
missing (TS2339).

Not covered yet: a class deriving from a member of an unresolved
`import X = require(…)` still inherits an open base in surge (tsgo reports the
missing members as TS2339); checked 2026-09-26.
