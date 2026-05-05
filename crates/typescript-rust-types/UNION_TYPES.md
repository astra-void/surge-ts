# Union Types

Current scope:
- `undefined`
- explicit union annotations
- conditional expression union inference
- optional property and optional element access widening
- nullish coalescing `??` evaluation removing `undefined`
- `keyof T` and `keyof typeof` type queries returning unions of string literal types
- `T[keyof T]` indexed access resolving to a union of property values

Normalization:
- flatten nested unions
- dedupe by type equality
- collapse single-member unions
- collapse unions containing `any` to `any`

Assignability:
- value to union target: assignable if it matches at least one constituent
- union source to target: assignable if every constituent is assignable to the target
- union source to union target: every source constituent must match at least one target constituent
- `keyof` intersections for unions are unsupported and fall back to `unknown`

Limitations:
- no narrowing
- literal union members exist and are used by `keyof`, but full TypeScript literal-union normalization/simplification and narrowing remain unsupported
- no `null`, `void`, or `never`
- no exact optional property semantics
