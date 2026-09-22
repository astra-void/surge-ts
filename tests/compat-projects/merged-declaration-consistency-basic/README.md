# merged-declaration-consistency-basic

Checks across the declarations of one merged name: exported and local
declarations sharing a declaration space (TS2395, tsc's
`checkExportsOnMergedDeclarations`), class and interface declarations whose
type parameter lists differ (TS2428, `checkTypeParameterListsIdentical`), and
an `export =` beside another value export (TS2309,
`checkExternalModuleExports`).
