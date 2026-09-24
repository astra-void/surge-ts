# optional-call-chain-nullability

tsc adds a chain's `undefined` to an optional call's result only when the
chain can short-circuit — when its receiver (`maybe?.run()`) or callee
(`maybeFn?.()`) can be nullish (`getOptionalExpressionType`). On a receiver
that cannot, `always?.run()` is exactly `always.run()`, and calling the
parenthesized result of such a chain is fine.
