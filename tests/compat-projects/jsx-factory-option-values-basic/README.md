# jsx-factory-option-values-basic

`jsxFactory` and `jsxFragmentFactory` must each parse as an identifier or a
qualified name (`parser.ParseIsolatedEntityName`): `"h."` is `TS5067` and
`"234"` is `TS18035`, both reported at the option's value in `tsconfig.json`
(`createOptionValueDiagnostic`). An options diagnostic stops the program
before its semantic diagnostics, so the JSX in `src/` reports nothing.
