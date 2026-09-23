# narrowing-in-keyword-presence

tsc's `narrowTypeByInKeyword` (flow.go): when some constituent of the tested
type can have the key (`isTypePresencePossible` — it declares the key or an
index signature answers it), the type is filtered by whether each constituent
can have the key in the branch at hand, which leaves `never` when none can —
a non-union type as much as a union. Otherwise only the true branch learns the
key, as an intersection with `Record<K, unknown>`. A numeric key names the
property its canonical spelling does (`1 in x` tests `"1"`).

surge filtered unions only, and kept the type whole when every member was
ruled out, so the `else` of `"a" in x` on `{ a: string } & { b: string }`, the
last branch of an exhausted `in` chain, and `!("a" in x)` on `{ a: string }`
were never `never`. A key no member knows was dropped from a union's true
branch instead of being learned.

The one error (`this.a` where `this` is `never`) is also tsc's.
