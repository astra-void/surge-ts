# computed-accessor-body-basic

A `get`/`set` accessor with a computed name (`get [key]()`,
`get [Symbol.toStringTag]()`) was dropped during lowering in both classes
and object literals, so its body went unchecked. It is now kept under the
same `[…]` name a computed method uses.
