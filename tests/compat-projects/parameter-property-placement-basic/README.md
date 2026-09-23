# parameter-property-placement-basic

An accessibility, `readonly` or `override` modifier on a parameter declares a
parameter property, which only a constructor with a body can have: tsc's
`checkParameter` reports TS2369 on every other parameter list (methods,
functions, overload and interface signatures, function types). Its modifier
grammar (`checkGrammarModifiers`) rejects only `static`, `export`, `declare`
and `async` on a parameter, as TS1090.
