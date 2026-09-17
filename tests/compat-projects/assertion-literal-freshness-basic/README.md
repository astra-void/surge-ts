# assertion-literal-freshness-basic

A `let`/`var` binding, or a parameter with a default, widens the literal type of
its initializer — but tsc widens only *fresh* literal types
(`getWidenedLiteralTypeForInitializer` → `getWidenedLiteralType`). An assertion
(`as const`, `as "q"`) yields the regular type, so `let s = "a" as const` keeps
the type `"a"` and a later `s = "b"` is TS2322. surge widened every initializer.

The negatives are pinned too: a plain literal initializer and a copy of a
`const` still widen, since both are fresh.
