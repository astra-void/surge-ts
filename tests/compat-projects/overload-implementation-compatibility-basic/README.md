# overload-implementation-compatibility-basic

tsc's `isImplementationCompatibleWithOverload` (TS2394 on the overload): the
return types must be related in either direction unless the overload returns
`void`, the implementation may not require more arguments than the overload
declares, and — with strict variance, as for any function declaration — each
overload parameter (with `undefined` when optional) must be assignable to the
implementation's. surge never checked overloads against their implementation.
