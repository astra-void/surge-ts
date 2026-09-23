# export-assignment-context-basic

An `export =` inside a function is TS1231 (`checkExportAssignment` via
`checkGrammarModuleElementContext`); at the top level of a CommonJS module it
is fine.
