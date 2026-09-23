# tuple-optional-element-flags-basic

tsc keeps a tuple element's optionality as a flag beside its type
(`ElementFlags`), and a tuple's `minLength` counts the required ones — which
the element types cannot always show. `[a?: any]` reads `any`, since `any`
absorbs the `undefined` an optional element adds; `[a: string, b: number |
undefined]` is required throughout although its last element takes
`undefined`.

surge read a tuple's optional elements off trailing slots that carry
`undefined`, so `[pattern: p, value?: any]` — ts-pattern's `isMatching` rest
parameter — had the `length` `2`, and `args.length === 1` was a false
TS2367. The parser now records the written `?` and the tuple keeps the
`minLength` it gives wherever the element types would read another one, for
`length`, relations and identity alike.

The errors from `notOne` down are intentional and are `tsc` errors too.
