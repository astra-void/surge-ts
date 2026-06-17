# Literal Types

This crate currently supports string, number, and boolean literal types.

## Behavior

- Literal expressions infer literal types.
- Implicit `let` and `var` declarations widen literal initializers to their primitive base type.
- Implicit `const` declarations preserve the literal type.
- Literal types are assignable to the same literal and to their primitive base type.
- Primitive base types are not assignable to narrower literal types.
- Unions keep the existing union-lite behavior: they flatten nested unions, dedupe exact matches, and do not absorb literal members into primitive members.

## Equality And Operators

- Equality checks compare literal overlap conservatively.
- Direct different literal comparisons are expected to report no overlap.
- Operators use primitive base behavior and do not evaluate literal arithmetic, concatenation, or other literal-specific results.

## Limitations

- v0.77 supports a narrow parser-safe `as const` foundation for primitive/object/array literals. Deep readonly semantics, readonly write restrictions, discriminated unions, template literal types, and full literal simplification remain unsupported.
- No template literal types.
- No narrowing.
- No discriminated unions.
- No literal simplification such as `"a" | string => string`.
