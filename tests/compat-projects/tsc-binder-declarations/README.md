# tsc-binder-declarations

The errors typescript-go's binder reports for one file: declarations that
cannot share a name (block-scoped redeclarations, conflicting kinds, an enum
merged with a non-enum, multiple default exports), strict-mode restrictions on
names in modules, classes and scripts, labeled declarations, `#constructor`,
misplaced `export as namespace` and an ambient module pattern with two `*`.

The `global-*` scripts and `augment.ts` cover the checker's merge of every
file's global declarations: scripts whose declarations conflict, interface
members that conflict across files, a module augmenting an ambient module, and
a script redeclaring `undefined`.
