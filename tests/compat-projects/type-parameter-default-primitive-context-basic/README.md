# type-parameter-default-primitive-context-basic

A type parameter no argument gives a candidate for takes its default
(`R2 = never` when the second callback is omitted) — unless the contextual
return type could have given it one. surge could only prove "the context
records nothing" for a *function* source, so under any other annotation
(`const a: boolean = chain.next(…)`) the default was withheld and the result
kept the raw name: `Chain<string | R2>`. tsc's `inferFromTypes` pairs a
primitive source through its wrapper interface (`Boolean`, `String`, …) and a
`undefined`/`null`/`void` source with nothing at all, so neither can name `R2`.
