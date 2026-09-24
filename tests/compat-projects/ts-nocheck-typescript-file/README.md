# ts-nocheck-typescript-file

A `// @ts-nocheck` line comment in a file's leading trivia makes tsc skip type
checking of that TypeScript file (`SkipTypeChecking` via
`canIncludeBindAndCheckDiagnostics`), so its semantic diagnostics — including
unused locals — disappear. The last `@ts-check`/`@ts-nocheck` pragma wins, and
a pragma after the first token is not one.
