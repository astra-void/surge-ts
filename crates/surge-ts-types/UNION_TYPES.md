# Union Types

Current scope:

- `undefined`
- explicit union annotations
- conditional expression union inference
- optional property and optional element access widening
- nullish coalescing `??` evaluation removing `undefined` and `void`
- `keyof T` and `keyof typeof` type queries returning unions of string literal types
- `T[keyof T]` indexed access resolving to a union of property values
- Homomorphic mapped types iterating over string literal unions to map properties
- utility-type lowering for `Record` and `Pick`/`Omit` key selection accepts only unions of string-literal keys in the narrow supported subset

## Normalization (`union_type` in `src/union.rs`)

`union_type(types)` is the single normalization entry point:

- flatten nested unions
- collapse any union containing `any` to `any`
- drop `never` members (`T | never` is `T`); a union whose members were all
  `never` is `never`, and an empty input stays `Unknown`
- dedupe by type equality, preserving first-seen member order
- collapse single-member unions to the member

Dedup is two-tier: at or below `LINEAR_DEDUP_LIMIT` (16) members a pairwise
scan wins; above it, members are bucketed by a coarse structural fingerprint
(`dedup_key`) with equality-confirmed buckets. The fingerprint maintains the
invariant that `a == b` implies `dedup_key(a) == dedup_key(b)` — it hashes
only fields that participate in `Type` equality, always structurally (never by
pointer, since `ObjectType`/`FunctionType`/`UnionType` equality accepts
structurally-equal values behind distinct `Arc`s), and combines
order-independent object property names commutatively.

## Allocation-reduced canonicalization

Normalization operates over *borrowed* members: flattening, `any`/`never`
handling, and dedup never clone a `Type`. When more than one member survives,
`UnionType::from_borrowed_members` probes the interner through the borrowed
slice (`ProgramTypeStore::intern_union_borrowed` in `src/store.rs`):

- **hit** (the overwhelmingly common case — measured 2.4M hits vs 89k unique
  unions on tRPC): the shared payload is returned and no member is cloned;
- **miss** (a genuinely new canonical union) or an over-budget fingerprint:
  the members are cloned once into the owned `intern_union` path.

## Canonical store behavior

`UnionType` is a handle (`Arc<UnionTypePayload>` + optional `UnionTypeId`)
whose payload holds the member list as `Arc<[Type]>`. Interning follows the
same pattern as function types:

- the member list is fingerprinted under a bounded budget; `Type::Unknown`,
  context-retaining references, and over-budget structures refuse
  fingerprinting and fall back to an uninterned payload with `id: None`;
- the fingerprint selects one of 64 mutex-guarded shards; the bucket `Vec` is
  scanned with exact canonical structural equality, so fingerprint collisions
  are harmless;
- store buckets are uncapped — bounding lives in the checker's program caches
  (`GENERIC_INSTANTIATION_BUCKET_CAP` in
  `surge-ts-checker/src/infer/types/cache.rs`), not in the type store;
- canonical unions are order-sensitive: `A | B` and `B | A` intern separately
  (member order is part of equality and of rendering), which is why
  normalization fixes first-seen order before interning;
- `UnionTypeId` embeds the store owner tag and must never cross program
  owners. The payload's `list_id` field is currently always `None` (union
  member lists are not separately interned the way function parameter lists
  are).

## Assignability

- value to union target: assignable if it matches at least one constituent
- union source to target: assignable if every constituent is assignable to the target
- union source to union target: every source constituent must match at least one target constituent
- `keyof` intersections for unions are unsupported and fall back to `unknown`

## Limitations

- flow narrowing over unions remains limited (see the checker's flow module
  for the supported truthiness/switch subset)
- literal union members exist and are used by `keyof`, but full TypeScript literal-union normalization/simplification (e.g. literal-absorbing-into-primitive reduction) remains unsupported
- `null` is not modeled as a distinct type; `void` and `never` exist as types
  and participate in normalization as described above
- no exact optional property semantics

## Clone accounting

- handle-copy measurements are attributed through `TypeCopyReason` and
  reasoned helper methods on callers, but the union-type representation itself
  remains handle-backed and semantically unchanged
