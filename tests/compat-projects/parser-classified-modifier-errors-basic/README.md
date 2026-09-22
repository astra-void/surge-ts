# parser-classified-modifier-errors-basic

Modifier and member grammar that oxc rejects with the TypeScript number tsc
uses (tsc reports them from `checkGrammarModifiers` and friends, so the rest
of the program is still checked). surge reads the modifier back out of oxc's
label where oxc words the message differently (TS1031 on a constructor,
TS1273), and re-anchors TS1248 on the member name and TS1545/TS1546 on the
`using` keyword the way tsc does.
