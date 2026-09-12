# relative-module-augmentation-heritage-basic

The other half of the `@typescript-eslint` `parent` cluster: a relative
`declare module "./generated/spec"` augmentation of a *base* interface, read
through a derived one (`Identifier extends BaseNode`).

`relative-module-augmentation-basic` files the augmentation under the target
file's identity and merges it into the export-table copy each importer
receives. That is enough when the consumer names the augmented interface
itself. Heritage resolves the base under the declaring file's own scope, which
read the file's own declaration table — the unmerged one — so `Identifier`
never saw `parent`, and which consumer had triggered the expansion decided
what a shared expansion contained.

The augmentation is now merged into the target file's own declaration table
when that table is collected, before any export table or heritage chain is
built from it. The importer-side application keeps only what that merge cannot
supply — the augmentation's values and brand-new declarations — so a body is
never merged twice.
