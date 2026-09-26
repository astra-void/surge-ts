# jsx-react-namespace-option-value-basic

Without a `jsxFactory`, `reactNamespace` must be an identifier
(`scanner.IsIdentifierText`): `"my-react"` is `TS5059`, reported at the option's
value in `tsconfig.json`. The options diagnostic stops the program before its
semantic diagnostics.
