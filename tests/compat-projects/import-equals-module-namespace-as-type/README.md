# import-equals-module-namespace-as-type

tsc's `checkAndReportErrorForUsingNamespaceAsTypeOrValue`: a name used as a type
that resolves with a `Module` meaning is TS2709 ("Cannot use namespace as a
type"), ahead of the value-as-type TS2749. `import x = require("./m")` over a
module with no `export =` is an alias of the module itself, and one over
`export = N` of a namespace `N` an alias of that namespace — both are
namespaces. surge only recognised `import * as` bindings and reported TS2749.

A `const` holding the module object is a value, not an alias, and stays TS2749.
