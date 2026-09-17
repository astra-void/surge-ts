# duplicate-export-specifier-basic

Two `export { … }` specifiers publishing one name are duplicate exported
identifiers. tsc reports every one of them — plus the exported declaration,
when there is one — on the exported name rather than on the specifier, and
words it as a block-scoped redeclaration (TS2451) instead of TS2300 when
that declaration is an exported `let`/`const`. Only the first specifier
merges with the declaration, so TS2323/TS2484 skip the later ones. surge
reported nothing here.
