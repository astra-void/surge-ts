# jsx-type-only-factory-import-basic

`import type React from "react"` over an `export =` module binds `React` in
type space only. Under `jsx: react` every tag reads the factory as a value, so
each reports TS1361, but the JSX namespace is still resolved through the
alias (`getJsxNamespaceAt` resolves the factory with the namespace meaning):
the intrinsic element's props are checked, and `React.ReactNode` resolves in
type positions.
