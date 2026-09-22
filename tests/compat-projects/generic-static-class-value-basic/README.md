# generic-static-class-value-basic

A class value is checked like any other value when its statics are generic
(`static deserialize: <T>(payload: string) => T`) or take a written `unknown`.
surge's "the shape is incomplete" guards read the signature's own type
parameters — and a parameter resolved over them, like `Omit<Transformer<I, O>,
'name'>` — and the `unknown` keyword as its degradation sentinel, so every
assignment, argument and return of such a class (superjson's `SuperJSON`) went
unreported.
