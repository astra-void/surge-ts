# rest-infer-capture-basic

`T extends (...a: infer A) => any ? A : never` against a variadic function
must capture the rest parameter's own array type: `Args<(...a: any[]) => void>`
is `any[]`. surge wrapped the check signature's remaining parameters in a
tuple, so the capture came out as `[any[]]` and every call through
`(...args: Args<Procedure>)` — vitest's `Mock` — rejected each argument
against `any[]`.

Fixed parameters ahead of a rest have no variadic tuple to land in here; they
widen into the element union. The two intentional errors pin that a fixed
capture and a typed mock still check their arguments.
