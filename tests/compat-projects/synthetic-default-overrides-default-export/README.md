# synthetic-default-overrides-default-export

tsc's `getTargetOfModuleDefault`: when `canHaveSyntheticDefault` holds, a
default import binds the module itself — its `export =` entity, else its
namespace — even if that entity has a `default` member. A TypeScript file
qualifies only through `export =` (`alias.ts` re-exports a module through
`import x = require()` + `export =`, `record.ts` assigns an object with a
`default` property); a declaration file qualifies unless it writes its own
default (`declDefault.d.ts`) or exports `__esModule` (`marked.d.ts`, TS1192).
The rule covers a default import, a `{ default as x }` specifier, and
`export { default } from`.
