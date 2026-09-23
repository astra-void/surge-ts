# annotated-destructuring-declaration

A destructuring declaration with a type annotation reads its elements from the
annotation (tsc's `getTypeForBindingElementParent` takes the declared type), so
`let { x }: { x?: string | number } = {}` is fine and `data` from an `any`
initializer is `string[]`. The initializer must be assignable to the annotation
(TS2322), and an element the annotation lacks is TS2339 on its name.
