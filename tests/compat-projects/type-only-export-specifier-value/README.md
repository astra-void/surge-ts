# type-only-export-specifier-value

An export specifier resolves its name in every meaning; `type` only restricts
how the export may be used. `export { type as }` exports the local `type`, and
`export { type something }` is a type-only export of the value `something`, not
an unresolved type, so importers can use it under `typeof`. A name with no
declaration at all is still TS2304.
