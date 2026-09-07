# assertion-and-nonnullable-basic

Two facts surge dropped.

An `asserts value is T` signature narrows from the *statement* onward rather
than inside a branch, so it has to be applied where the expression statement is
checked — the guard machinery only ever looked at conditions. `asserts value`
with no target proves only truthiness, and is applied as such.

`NonNullable<T>` is defined as `T & {}`, and an object operand excludes
`null`/`undefined`, so `undefined & {}` is `never`. Without that reduction the
brand-collapse handed `undefined` straight back and every `NonNullable<…>` kept
its `| undefined` — which is what made a spread of `Required<NonNullable<…>>`
defaults contribute optional properties instead of required ones.

`void` is deliberately left out of the reduction: surge models
`PromiseLike<void>` as its awaited `void`, so reducing `void & { … }` to `never`
would strip the contextual type off methods written against that intersection.

The single intentional error is the last function: with no assertion the
receiver is still `unknown`.
