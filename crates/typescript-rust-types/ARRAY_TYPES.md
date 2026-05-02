# Array Types

Supported surface:

- `T[]` in type positions
- array literals like `[]` and `["Ada", "Grace"]`
- numeric index access like `values[0]`
- element inference from array literals
- contextual checking of array literal elements against `T[]`

Limitations:

- no tuple types
- no `Array<T>`
- no `ReadonlyArray<T>`
- no readonly arrays
- no array methods
- no spreads or holes beyond basic trailing-comma parsing
- no generic array parsing
- no lib.d.ts modeling
