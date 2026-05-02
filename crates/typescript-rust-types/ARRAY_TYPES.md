# Array Types

Supported surface:

- `T[]` in type positions
- array literals like `[]` and `["Ada", "Grace"]`
- numeric index access like `values[0]` on simple identifier receivers
- array literal inference that preserves literal element types and unions
- contextual checking of array literal elements against `T[]`
- `any` array elements collapse inference to `any[]`
- `any` index receivers return `any` and skip index checking
- `unknown` receivers remain shallow and do not emit an index diagnostic
- unresolved or unknown array literal / index elements stop with `unknown` to avoid cascades
- tuple arrays and tuple-specific behavior are documented in `TUPLE_TYPES.md`
- tuples are still inferred only when they are contextual, not from bare array literals

Limitations:

- no `Array<T>`
- no `ReadonlyArray<T>`
- no readonly arrays
- no array methods
- no property index access
- no nested index access
- no index calls
- no spreads or destructuring
- no lib.d.ts modeling
