# default-and-namespace-import

`import greeting, * as module from "./m"` binds both the default export as
`greeting` and the module namespace as `module`: `module.other` and
`module.default` resolve, and a missing member reports TS2339 on the namespace
type.
