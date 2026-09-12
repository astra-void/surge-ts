# synthetic-default-import-type-members-basic

`import http from "http"` under `esModuleInterop` binds the module object as
the default, so its exported types are reachable as `http.RequestListener`
exactly as through `import * as http`. surge bound the synthetic default's
*value* but registered no type members for it, and a qualified type name it
cannot resolve is deliberately silent (no TS2304, to stay quiet on `@types/*`
namespaces) — so every `http.X` annotation degraded to the sentinel and a call
through vitest's `Mock<http.RequestListener>` was a false TS2349 in tRPC.

The synthetic default now also contributes the module's exported types as a
`local.<member>` alias layer, the same layer a namespace import gets. The
errors pin that the members really bind: an interface's property type and a
class instance type read through the qualified name.
