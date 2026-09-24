# conditional-expression-subtype-reduction-basic

tsc types `c ? a : b` as `getUnionTypeEx([a, b], UnionReductionSubtype)`
(`checkConditionalExpression`): a branch whose type is a strict subtype of the
other's is removed (`removeSubtypes`, relater.go `strictSubtypeRelation`). The
strict subtype relation orders what assignability leaves mutual: a signature
with fewer parameters is the subtype of one with more optional ones, and a
required `x: T | undefined` parameter is the subtype of an optional `x?: T`,
so `cond ? requiredUndefined : optionalArg` is callable with no argument while
`cond ? requiredHello : optionalArg` is not. An interface is absorbed into the
one it extends, and an optional member of the other branch must be present.

Two fresh object literals stay apart: a literal's excess property keeps it
from being a subtype of the other literal, whose missing required member keeps
it from being one in turn.

Every error below is also a `tsc` error.
