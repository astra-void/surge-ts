# numeric-enum-number-assignability-basic

Any `number` is assignable to a numeric `enum` — the enum type and a member
type alike. That is not a courtesy: `Flags.Read | Flags.Write` is itself typed
`number`, so combining flags and passing the result is the normal way to call
such an API, and rejecting it makes every flag argument a false `TS2345`.

A *string* enum takes no `string`, which is the only diagnostic here.

surge lowers an enum to a type alias over its member types and wraps the
resolution in a nominal reference; the reference now records whether the enum
is numeric, because once the members resolve the union is indistinguishable
from any other union of number literals. tRPC's `openapi` package reaches this
through `ts.TypeFlags`.
