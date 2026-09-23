# export-clause-local-targets-basic

A local `export { … }` names any local of the module (tsc's
`checkExportSpecifier`): a `var` hoisted out of a module-level block, loop,
`for…in` head or `try`/`catch`; a namespace that has only a namespace meaning;
an import binding; an `import x = N.M` alias, which exports every meaning of
the entity it names (and so does `export import x = N.M`). A name that resolves only to a global declaration is not a
local to export (TS2661); a name that resolves to nothing is TS2304.
