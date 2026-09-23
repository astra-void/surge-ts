# class-property-arrow-signature

An unannotated class property initialized with an arrow or `function`
expression that writes its return type has the signature it spells
(`getWidenedTypeForVariableLikeDeclaration` of the function expression). A
class declaration's initializer has no contextual type, so an unannotated
parameter is `any` and a defaulted one takes its initializer's widened type,
optional. The static side had only literal initializers typed, so
`static create = (seed?: number): Factory => …` — the factory shape zod v3
declares every schema constructor with — read as `any`, and so did every value
exported from it.
