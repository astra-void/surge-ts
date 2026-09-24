# export-star-as-default

`export * as default from "./m"` is the module's default export: its value is
`m`'s namespace object, and tsc's `isSyntacticDefault` counts the namespace
export, so a declaration file writing it has no synthetic default either.
