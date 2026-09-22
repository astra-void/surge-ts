# promise-relation-basic

A value that cannot be a thenable is not assignable to `Promise<T>`, and a
`Promise<T>` is not assignable to a primitive. surge reads a promise as the
value it resolves to, which related the two in both directions. An async
function relates the awaited value to the awaited return type
(`unwrapReturnType` / `checkAwaitedType`), so returning either `T` or
`Promise<T>` stays valid. `await` written directly as a call argument was
dropped by the parser and the argument kept its promise type.
