# namespace-block-interface-merging-basic

tsc's binder gives every block of a namespace one symbol, and
`declareModuleMember` declares an exported member in that symbol's export
table, so an interface re-opened in another block — of the same file or, for a
global namespace, of another file — is one merged interface. Name resolution
inside a block reaches it through the namespace's exports, so each block sees
the members every block declares. A member written without `export` lives in
its own block's locals instead and merges only within that block; in an
ambient block without export declarations every member is exported
(`setExportContextFlag`).

surge registered one block's declaration first-wins, or let a block that
re-opened a name twice replace the others, so every block but one reported
false TS2339s on members declared elsewhere. The intentional errors are the
reads of members no block declares.
