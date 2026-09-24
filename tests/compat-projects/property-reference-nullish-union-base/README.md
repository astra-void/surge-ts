# property-reference-nullish-union-base

`node.init !== null` narrows the reference `node.init` for the right operand
of `&&` as it does in an `if`, including when `node` is a union: each
member's `init` loses its `null`, and one that is only `null` becomes
`never`. The expression-level narrowing handled only an object base.
