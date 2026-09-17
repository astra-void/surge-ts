# this-parameter-typing-basic

`function f(this: T)` types `this` as `T` in the body. The parser noted that a
`this` parameter was written (to clear the implicit-`this` report) but dropped
its annotation, so `this` had no type: a mistyped read, a read or write of a
member `T` does not declare, and every other use went unchecked.
