# Function Types

The type system supports a minimal callable surface for annotations and type aliases.

Supported syntax:

- `() => T`
- `(value: T) => U`
- multiple parameters
- nested function types in parameter and return positions
- `void` return types

## Representation

A `FunctionType` (`src/function.rs`) is a cheap handle: an
`Arc<FunctionTypePayload>` plus an optional canonical `FunctionTypeId`. The
payload holds the structural facts only:

- `parameters: Arc<[Type]>` (optionally backed by a canonical `TypeListId`)
- `return_type: Type`
- `is_variadic: bool` — rest-argument acceptance (used by synthetic built-ins
  to avoid rest-parameter false positives)
- `required_parameter_count: usize` — minimum call arity, which models
  trailing optional parameters

Parameter names, default values, and `this` parameters are not represented.
Cloning a `FunctionType` copies the handle (an `Arc` bump), never the payload;
`FunctionTypePayload::clone` (a true deep clone) is separately counted and
should stay rare.

## Construction lifecycle

`FunctionType::new` runs the canonical-store path when a program store is
installed (see `src/store.rs`):

1. **Construction request** — the caller passes owned parameters/return type.
2. **Canonical key** — the store fingerprints the parameter list and return
   type under a bounded budget (`FingerprintBudget`: limited node count and
   depth). Fingerprinting *refuses* `Type::Unknown` (the degradation
   sentinel), references that retain resolution context or opt out of program
   canonicalization, and over-budget structures.
3. **Store lookup** — the fingerprint selects a shard/bucket; the bucket is
   scanned with exact canonical structural equality (`canonical_types_equal`),
   so a fingerprint collision can never return a wrong payload. The parameter
   list is interned first (`intern_parameter_list`), so distinct functions
   with identical parameter lists share one `Arc<[Type]>`.
4. **Canonical payload or fallback** — a hit returns the shared payload and
   its `FunctionTypeId`; a miss allocates the payload once and registers it.
   If fingerprinting refused the value (or no store is installed — the
   single-file path and unit tests), the constructor falls back to a fresh
   uninterned `Arc` payload with `id: None`. Fallbacks are semantically
   identical, just unshared; the store counts them (`function_fallbacks`).

IDs embed the store's owner tag: a `FunctionTypeId` is only meaningful with
the program store that minted it and must never cross program owners.

The store also memoizes overload merges: `lookup_overload_merge` /
`record_overload_merge` cache the merged permissive signature for a pair of
canonical function IDs (held via `Weak`, so the cache never keeps a payload
alive on its own).

## Structural identity vs diagnostic source identity

Payload identity is deliberately **structural**: two unrelated declarations
with the same shape (`(x: string) => number` declared in two files) intern to
the same payload and ID. That is safe because nothing diagnostic-facing hangs
off the payload — display renders the structural shape (`FunctionType::name`),
and equality (`Arc::ptr_eq` fast path, then structural comparison) is
shape-based.

Declaration/source provenance is a separate concern owned by the checker:
which interface member a signature came from, its declaration span, and its
overload position are tracked in checker-side keys
(`StableInterfaceMemberDeclarationId`, overload group templates in
`surge-ts-checker/src/context.rs`), never inside `FunctionTypePayload`.
Keeping the two apart is what makes structural interning sound: sharing a
payload across declarations cannot change which declaration a diagnostic
points at, and overload order/duplicates are preserved by the checker's
ordered templates rather than by payload identity.

## Assignability policy

- user-authored function types remain exact-arity, while synthetic built-ins can use the internal `is_variadic` flag to avoid common rest-argument false positives
- parameters use a conservative compatibility check
- return types must be assignable

## Limitations

- `void` is intentionally minimal and only models the current checker surface
- function-type parameter lists may carry parsed type parameters, defaults, and
  constraints, but the checker only performs narrow call-site instantiation for
  simple direct calls; full generic inference, overload resolution, callback
  contextual inference, higher-order inference, and tuple-valued implicit
  generic returns remain unsupported
- parameter optionality is modeled only as `required_parameter_count`
  (trailing optional arity) and rest acceptance only as `is_variadic`;
  per-parameter names, default values, and `this` parameters are not
  represented
- callable/constructable *objects* are modeled on `ObjectType` via optional
  `call_signature` / `construct_signature` fields (see `src/object.rs`), not
  on `FunctionType` itself
- unions containing function types are allowed as types, but callable union semantics are not implemented yet
- no strict TypeScript variance fidelity yet

## Clone accounting

- handle-copy measurements are attributed through `TypeCopyReason` and
  reasoned helper methods on callers, but the function-type representation
  itself remains handle-backed and semantically unchanged
- payload allocations are additionally attributed to a thread-local expansion
  reason (`replace_function_type_expansion_reason`), surfaced by the
  `--timings` counters
