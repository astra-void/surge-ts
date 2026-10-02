// @types: dep
// @filename: node_modules/dep/index.d.ts
// Two instantiations of the same degraded alias resolve independently — a
// degraded resolution is never cached — and the unresolvable name is still
// reported exactly once. The declaration file is an ambient script (no
// top-level import/export): a *module* declaration file is module-scoped, so it
// is not lowered by the ambient-global passes at all.
type Broken<T> = Missing<T>; declare const first: Broken<string>; declare const second: Broken<number>;
// @filename: src/index.ts
first.anything; second.anything;
