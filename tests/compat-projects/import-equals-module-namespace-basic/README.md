# import-equals-module-namespace-basic

`import x = require("./m")` where `m` is an ordinary module — no `export =` —
binds the module's namespace, exactly as `import * as x` does. surge bound an
unknown placeholder instead, so every `x.member` and every `typeof x.member`
went silent, including the missing-export error tsc reports.

tRPC's `upgrade` transforms are the corpus shape: `@types/jscodeshift` writes
`import recast = require("recast")` and builds its whole `JSCodeshift` surface
out of `typeof recast.types.namedTypes` / `typeof recast.types.builders`, none
of which resolved.

The namespace member (`shapesModule.shapes.Box`) is deliberately not asserted
here: a `declare namespace`'s *value* members are still built permissively, so
its type is `any` and a mismatched use reports nothing. That is a separate gap.
