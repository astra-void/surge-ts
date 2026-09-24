# error-call-operands-checked

tsc checks what a call's operands name even when the call itself is an error
(`resolveErrorCall`): the arguments of a call to an unresolved name or a
missing member, with no contextual type, and the type arguments of an untyped
call. It checks an element access's index before giving up on an error
object, and a `for...in` head that names nothing. The iterator an array's
`values()` returns is the lib's `ArrayIterator`, with the `IteratorObject`
helpers the configured lib declares.
