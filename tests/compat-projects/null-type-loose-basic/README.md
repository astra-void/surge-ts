# null-type-loose-basic

Without `strictNullChecks`, `null` and `undefined` are in the domain of every
type and widen to `any` wherever a declaration's type is inferred
(`{ p: null }` is `{ p: any }`), so none of this reports except the one real
mismatch. (`strict` is off as a whole: with `noImplicitAny` on, tsc also
reports each such widening as TS7005/TS7018, which surge does not model yet.)
