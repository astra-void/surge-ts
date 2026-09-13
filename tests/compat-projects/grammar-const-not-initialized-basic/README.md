# grammar-const-not-initialized-basic

`const pending: number;` is a grammar error (`TS1155`) whatever the type
annotation says, and surge reported nothing for it: the lossy `Parsed*` tree
keeps no "was there an initializer" answer the checker could act on, so the
check runs in the parser's grammar walk instead.

The rest of the file pins the exemptions that walk has to honor: a `declare
const`, a `const` inside a `declare namespace`, and the two loop heads whose
binding is initialized by the loop rather than by an initializer.
