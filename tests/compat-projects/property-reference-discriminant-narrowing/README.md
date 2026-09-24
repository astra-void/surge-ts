# property-reference-discriminant-narrowing

`node.init?.type === Kind.Identifier` narrows the reference `node.init` both
by the discriminant and by the chain's non-nullishness — in an `if`, and for
the right operand of `&&` and the true branch of `?:`. When `node` is a union
(typescript-eslint's `VariableDeclarator` is), each member's `init` narrows,
one that is only `null` to `never`, so a read of `node.init` sees what tsc's
narrowed reference does. Surge's expression-level narrowing skipped the
chain's non-nullishness, and every level skipped a union base.
