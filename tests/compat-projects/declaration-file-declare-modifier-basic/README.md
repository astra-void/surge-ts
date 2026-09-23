# declaration-file-declare-modifier-basic

`checkGrammarTopLevelElementsForRequiredDeclareModifier`: with `skipLibCheck`
off, a top-level declaration in a `.d.ts` file without `declare` or `export`
is TS1046 on its first token — once per file, on the first such declaration.
Interfaces, type aliases and exported or ambient declarations are fine.
