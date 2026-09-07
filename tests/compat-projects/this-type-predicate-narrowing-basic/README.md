# this-type-predicate-narrowing-basic

A method declared `isUnion(): this is UnionTy` narrows its *receiver*, which is
the shape TypeScript's own `ts.Type` API is built on. Resolving a signature
drops the predicate — `x is T` and `this is T` both resolve to `boolean` — so
the target has to be read back off the receiver's declaring interface, under
that declaration's own file and namespace scope.

The positive cases pin the four positions the narrowing has to survive: the
fall-through of an early-returning `!type.isIntersection()`, an `if` branch, a
call argument, and the right operand of an `&&`.

The single intentional error is the last function: the `else` branch proves
nothing, so `type.types` there is still a `TS2339` on `Ty`.
