# Function Types

The type system supports a minimal callable surface for annotations and type aliases.

Supported syntax:

- `() => T`
- `(value: T) => U`
- multiple parameters
- nested function types in parameter and return positions
- `void` return types

Assignability policy:

- user-authored function types remain exact-arity, while synthetic built-ins can use the internal `is_variadic` flag to avoid common rest-argument false positives
- parameters use a conservative compatibility check
- return types must be assignable

Limitations:

- `void` is intentionally minimal and only models the current checker surface
- function-type parameter lists may carry parsed type parameters, defaults, and
  constraints, but the checker only performs narrow call-site instantiation for
  simple direct calls; full generic inference, overload resolution, callback
  contextual inference, higher-order inference, and tuple-valued implicit
  generic returns remain unsupported
- no optional, rest, or default parameters
- no `this` parameters
- no methods or call signatures
- no arrow/function expressions
- no property call expressions
- unions containing function types are allowed as types, but callable union semantics are not implemented yet
- no strict TypeScript variance fidelity yet

Clone accounting:

- handle-copy measurements are attributed through `TypeCopyReason` and
  reasoned helper methods on callers, but the function-type representation
  itself remains handle-backed and semantically unchanged

Canonical-store retention:

- the canonical stores in `store.rs` (function payloads, parameter lists,
  unions, property maps) hold `Weak` payload references: a payload lives
  exactly as long as some consumer holds its `Arc`, and the store never keeps
  the whole type graph alive for program lifetime
- canonical IDs are monotonic within a program owner and never reused, so an
  expired entry cannot ABA into a later payload; ID-equality fast paths stay
  sound, and re-interning an equivalent payload after expiration allocates a
  fresh ID
- expired bucket entries are swept opportunistically on the next bucket scan;
  cleanup must stay deterministic and no lock may be held across recursive
  canonicalization of child types
- do not restore strong retention in a store, and do not tie payload lifetime
  to cache pruning: expansion-cache lifetime is semantically load-bearing
  (see `crates/surge-ts-checker/MEMORY_REGIONS.md`), and only true-death
  reclamation — an entry dying because no consumer exists — is approved
