# type-only-import-equals

`import type A = require("./a")` is an alias of every meaning the module's
`export =` entity has, marked type-only: using it in type position (and under
`typeof`) is fine, and a value use is TS1361. `import type = require("./b")`
is an ordinary import of a local named `type`. surge dropped the `type`
modifier, so it neither reported the value uses nor bound the type side.
