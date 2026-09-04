# ambient-script-var-global-augmented-type

A *script* declaration file's `declare var` is lowered against the ambient type
table, and its annotation's interface may be re-opened from a `declare global`
block in a sibling *module* declaration file. `@types/node` is exactly this
shape: `globals.d.ts` (a script) declares `var process: NodeJS.Process` while
`process.d.ts` (a module) contributes the interface's 87 members inside
`declare global`.

The two collections used to run in the wrong order — the script pass lowered the
value before the `declare global` types were merged — so `process` froze against
whatever partial `NodeJS.Process` happened to exist. With any local augmentation
present (`next/types/global.d.ts` adds a single `browser` member) that partial
declaration was the *one-member* one, and every real member read as missing.

The fixture reproduces it without Node: a script `globals.d.ts` declaring the
variable, a module `runtime.d.ts` contributing members through `declare global`,
and a project-local one-member augmentation. The last line keeps the members
honest — they resolve to their real types, so assigning `string` to `number`
still reports.
