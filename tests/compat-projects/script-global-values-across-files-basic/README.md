# script-global-values-across-files-basic

Every script's top-level `var`/`let`/`const` and namespace is declared in the
one global table, so another script reads it: `let greeting` in one file is
in scope in the next, and a namespace reopened in several scripts is one
merged namespace. surge only shared `declare`d values and functions/classes,
so these reads were a false TS2304 — and cross-file redeclarations went
unreported. TS2403 compares a redeclaration with the first declaration, so
only the later file reports it; TS2451 reports in both.
