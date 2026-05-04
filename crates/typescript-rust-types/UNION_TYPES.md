# Union Types

Current scope:
- `undefined`
- explicit union annotations
- conditional expression union inference
- optional property access widening
- nullish coalescing `??` evaluation removing `undefined`

Normalization:
- flatten nested unions
- dedupe by type equality
- collapse single-member unions
- collapse unions containing `any` to `any`

Assignability:
- value to union target: assignable if it matches at least one constituent
- union source to target: assignable if every constituent is assignable to the target
- union source to union target: every source constituent must match at least one target constituent

Limitations:
- no narrowing
- no literal types
- no `null`, `void`, or `never`
- no exact optional property semantics
