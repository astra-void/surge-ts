# Function Types

The type system supports a minimal callable surface for annotations and type aliases.

Supported syntax:

- `() => T`
- `(value: T) => U`
- multiple parameters
- nested function types in parameter and return positions
- `void` return types

Assignability policy:

- arity must match exactly
- parameters use a conservative compatibility check
- return types must be assignable

Limitations:

- `void` is intentionally minimal and only models the current checker surface
- function-type parameter lists may carry parsed type parameters, defaults, and
  constraints, but there is no generic inference or instantiation-lite for
  function-type parameters
- no optional, rest, or default parameters
- no `this` parameters
- no methods or call signatures
- no arrow/function expressions
- no property call expressions
- unions containing function types are allowed as types, but callable union semantics are not implemented yet
- no strict TypeScript variance fidelity yet
