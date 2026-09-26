# import-attribute-value-literals

An import attribute value must be written as a string literal (TS2858,
`checkImportAttributes`), even when its type is `string`: a no-substitution
template literal and a call returning a string are both reported.
