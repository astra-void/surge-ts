# Tuple Types

Supported surface:

- tuple type syntax like `[T, U]`
- empty tuples like `[]`
- tuple arrays like `[string, number][]`
- contextual array literal checking against tuple types
- numeric literal index access into tuples
- optional numeric literal index access like `tuple?.[0]` returning `T | undefined`
- tuple numeric indexed access types like `Tuple[0]`
- non-literal number index access returns the union of tuple element types
- tuple values are assignable to compatible arrays
- arrays are not assignable to tuples

Limitations:

- no readonly tuples
- no optional tuple elements
- no rest tuple elements
- no labeled tuple elements
- no variadic tuples
- no tuple destructuring
- no tuple methods or length property
- no property index access
- no nested index access
- no generic indexed access types
- no lib.d.ts modeling
- no generics, variadic tuples, or tuple destructuring
