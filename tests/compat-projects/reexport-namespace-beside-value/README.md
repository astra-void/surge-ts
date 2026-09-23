# reexport-namespace-beside-value

`export { Merged } from "./merged"` re-exports every meaning of `Merged`: the
function and the namespace merged with it. A namespace's members travel as
qualified `Merged.Member` types, which surge copied only when the name had no
value or type of its own — so the function hid the namespace and
`Merged.Member` did not resolve through the barrel. React 19's
`jsx-runtime.d.ts` re-exports its `JSX` namespace from the `export =` module
the same way.
