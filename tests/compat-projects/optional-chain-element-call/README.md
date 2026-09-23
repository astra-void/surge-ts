# optional-chain-element-call

A call continuing an optional chain invokes the member as read off the
non-nullish receiver (`getOptionalExpressionType`), so `maybe?.["run"]()`,
`maybe?.[key]()` and `deep?.inner["run"]()` are not TS2722 — only a member
that is itself optional is. The chain's short-circuit adds `undefined` to
the call's result instead (TS2322 for `tooNarrow`), and only when the chain
can short-circuit: `present?.["run"]()` is a plain `string`.
