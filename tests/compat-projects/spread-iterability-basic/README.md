# spread-iterability-basic

tsc requires the operand of an array spread, a call spread and `for…of` to be
iterable (TS2488); a spread of `undefined` or `unknown` is itself the error,
while `for…of` reports those as nullish/unknown operands. surge had no
iterability check at all.

A contextually typed `(...args)` over a signature with optional parameters
must bind the parameter tuple: surge bound the first parameter's type, so
zod's `inst.min = (...args) => inst.check(_minSize(...args))` spread a
`number`. `for await` may iterate an async iterable and is not checked here.
