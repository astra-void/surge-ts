# inference-nested-union-alias-basic

Substituting a union for an alias's parameter (`void | C` into `MaybePromise<T> =
T | Promise<T>`) writes a union inside a union. tsc's `getUnionType` flattens it
on construction, so `inferToMultipleTypes` still sees the naked `C` and infers it
from a callback's return. surge's inference walked the nested union as one
structured member, found no naked parameter, and inferred nothing (konn's
`beforeEach(() => ctx)`).
