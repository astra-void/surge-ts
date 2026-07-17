# tRPC allocation-volume program — engineering report

Branch `perf/trpc-allocation-volume`, started from `perf/trpc-5s @ 5515b68`.

## Outcome summary

Definitive same-window 4-way interleave (4 rounds, one thermal window,
old/new x system/mimalloc; raw log in `TRPC-ALLOCATION-VOLUME-MATRIX.txt`):

| config | wall median | user median |
|---|---|---|
| 5515b68 system | 15.38 s | 13.59 s |
| 5515b68 mimalloc | 11.87 s | 9.99 s |
| **final system** | **14.06 s** | **12.50 s** |
| final mimalloc | 11.38 s | 9.51 s |

- system allocator: **−1.32 s wall (−8.6%), −1.09 s user**
- mimalloc: −0.49 s (the removed clone/hash work is CPU, not only allocator time)
- system-vs-mimalloc gap: 3.51 s → 2.68 s = **−24%**
- diagnostics: 2,190, sha256 `4d69a2d5…`, byte-identical everywhere
  (system/mimalloc, jobs=1/auto, all 10 matrix runs, zod/ky/ofetch raw cmp)
- memory: internal peak fp 1.95–1.99 GB (required ≤ 2.00 ✓, preferred 1.90 ✗
  by +50–90 MB of Arc headers); finish fp **455–477 MB** (both gates ✓,
  −115 MB vs baseline)

Gate assessment (honest):

- required jobs=1 ≤ 13.0 s: **not met** (14.06 same-window; cool-window best
  13.46–13.79). The target implied −2.5 s from the 15.5 s reference; this
  program delivered −1.3 s.
- required gap reduction ≥ 40%: **not met** (−24%). An earlier cross-window
  comparison suggested −65–74%; the controlled 4-way interleave corrects it.
- memory required: **met**. correctness: **all met** (nextest 1546/1546,
  oracle sweep 97/97 zero gating drift, real projects 28/0/3, jobs parity,
  determinism).

Wall-time absolute numbers moved ±1 s between thermal windows this session;
every claim above is from a single interleaved window.

## Environment

Same machine/toolchain as `TRPC-5S-REPORT.md` (M1 Pro 10-core, 16 GB, macOS
27.0, rustc 1.94.0). Session note: ambient thermal load was higher and rose
through the session; every comparative claim below is from *interleaved*
old/new runs in one window. Absolute medians drifted ±1 s between windows.

## Stage 0 — baseline reproduction (this session)

```text
system jobs=1:    17.17 17.61 17.01 16.71 15.48   median 17.01 (thermally loaded window)
system jobs=auto: 15.89 17.24 16.11 15.90 14.96   median 15.90
mimalloc jobs=1:  12.64 13.23 13.71               median 13.23
gap (same-day):   ~3.8 s wall / ~3.6 s user
peak fp:          2.00–2.11 GB (time -l), 1.90 GB internal fp_peak
diagnostics:      2,190, sha256 4d69a2d5…, all runs
```

Allocation attribution (sample-based malloc-caller aggregation): allocator
self-time ≈ 22–24%; top owners: interface member resolution, Vec growth
(`RawVecInner`) across resolve/parse/diagnostics, union/function/reference
type construction, display-string building (`Type::name`,
`parsed_type_display`, join/format), `TypeParameterSubstitution::set`,
SipHash on `type_dedup_fingerprint` + loader caches.

## Stage 1 — ParsedType clone census (`SURGE_ALLOCATION_CENSUS=1`)

Landed as `5d05b06` (per-variant relaxed counters inside a manual `Clone`;
one predicted branch when disabled). tRPC per run at baseline:

```text
named 21.4M  primitive 14.7M  union/intersection/tuple 3.8M  function 2.8M
string-literal 2.1M  array/keyof 0.7M  object 0.36M  indexed 0.50M
mapped/conditional 0.34M  total 46.9M node clones
```

Composite clones ≈ 32 M, each 1–3 mallocs recursively at baseline — the
dominant identified share of the allocator gap.

## Changes retained

1. `beb33d5` **Arc-shared ParsedType composite payloads** (the centerpiece).
   `Named`, `Function`, `Union`, `Intersection`, `Tuple`, `Array`, `KeyOf`
   payloads moved behind `Arc`; clones are refcount bumps and never recurse;
   `resolve_named_type`/`resolve_function_type` take the `Arc` directly and
   clone only the fields that escape into lazy references (shallow
   `type_arguments`). Cool-window interleaved A/B: 15.4 → 13.7 s; controlled 4-way window attributes ≈−0.9 s of the total −1.3 s to this change.
   Census: 46.9 M → 33.0 M events, remaining events shallow. Finish footprint
   −115 MB; peak +90 MB (headers).
2. `8161a72` **Small-allocation churn batch.**
   - `TypeParameterSubstitution`: `Arc<BTreeMap<String, Type>>` →
     name-sorted `Arc<Vec<(Arc<str>, Type)>>` (+ placeholder vec); every
     `set()` against a shared substitution previously deep-copied the whole
     map. Iteration order (sorted) preserved exactly; the struct has no
     `PartialEq`, so no cache identity depends on representation.
   - Interface inheritance merge: probe before cloning
     (`entry(name.clone()).or_insert(property.clone())` paid both clones on
     already-shadowed members).
   - Last hot SipHash maps → Fx (`utility_diagnostic_keys`, loader probe +
     import-graph caches); membership-only sets, order-invisible.
   Interleaved A/B (cumulative with 1): user −0.6…−1.1 s in-window.
3. `a2c7247` **PropertyMap keys interned as `Arc<str>`** (the report's #1
   named follow-up lever). A derived interface inheriting its bases' members
   allocated a fresh `String` per inherited key even though the base maps are
   `Arc`-shared and reused across every derived type; the key clone is now a
   refcount bump and derived types share one allocation per base member name.
   Equality by str content (order-independent, unchanged), `Arc<str>: Hash`
   matches `String::hash` byte-for-byte → fingerprint / union dedup key /
   canonical property-map store untouched; the assignability failure enum keeps
   its `String` fields (cold path). Interleaved A/B vs `b9d1334` (jobs=1, one
   window): wall 14.68 → 14.58 s, **user 12.66 → 12.55 s (−0.9%), and all 5
   paired NEW user times below their OLD counterpart** — small but robust.
   Memory neutral within noise (gates hold). This confirms the program's
   recurring finding: the property-map allocation was a smaller malloc owner
   than its ~millions of clone *events* implied — clone-count censuses
   overstate malloc impact because most counted clones are cheap or already
   `Arc`-shared through the canonical store.

## Changes evaluated and declined (with evidence)

- **bumpalo / worker-local bump scratch: declined.** After the eliminate-work
  stages, the top remaining allocation owners all *escape* their scope:
  interface property maps become `Arc<PropertyMap>` inside retained
  `ObjectType`s, union/function/reference construction feeds the canonical
  weak stores, display strings land in diagnostics and type names. None are
  scratch-lifetime, and the mission's own rules (correctly) forbid placing
  Arc/String-owning escaping values in a resettable bump region. The
  remaining true-scratch paths (recursion stacks, candidate buffers) no
  longer register meaningfully in malloc-caller profiles. The spike would
  not have met its acceptance gate.
- **`type_dedup_fingerprint` hasher: untouched** (pinned; bucket composition
  is semantically load-bearing — see prior program).

## Remaining allocation owners (post-`a2c7247` profile)

```text
malloc self-time flattened; remaining owners are pinned or structural:
RawVecInner (Vec growth)          144 malloc-parent samples (resolve/parse/
                                  diagnostics temporaries; diffuse)
fingerprint_type SipHash          161 self samples (PINNED — bucket
                                  composition sets Arc pointer identity that
                                  assignability ptr_eq fast-paths depend on;
                                  Fx-hashing it drifted 1292 diagnostics)
interface member resolution        43 malloc-parent + 120 self (IndexMap
                                  backbone alloc; the key Strings are gone now)
FunctionType::new / union_type /
TypeReference::new                 ~58 (Type-layer payloads that feed the
                                  canonical weak stores — not scratch-lifetime)
display strings (Type::name,
parsed_type_display, join)         ~37 (byte-exactness-sensitive; risky)
TypeParameterSubstitution::set      24 (sorted-vec growth; entries are 1–3, tiny)
memmove                           377 (parser + Vec copies)
```

**Returns have flattened.** After three landed changes the remaining owners
are either semantically pinned (the fingerprint hash, display strings) or
structural Type-layer construction that feeds the canonical stores and so
cannot move to scratch/arena allocation. No large *reducible* allocation owner
remains that would not risk diagnostics.

Next-step recommendation: (1) the serial allocation headroom is now largely
consumed — the highest-value remaining work is the **parallel** win via
speculative transactional checking (see `TRPC-5S-REPORT.md`), which the
allocation program was scoped to exclude; (2) if pursuing more serial memory,
`Arc<[T]>` for the three `ParsedType` list variants would drop the inner `Vec`
header (recovers part of the +Arc-header peak); (3) reducing named clone
*events* (13.2 M refcount atomics) by threading `Arc<ParsedNamedType>` deeper
into the lazy-reference captures is a micro-optimization with unclear payoff.

## Memory safety / lifetime notes

No new arenas, no reset semantics, no Drop-skipping structures were added;
the Arc conversion only extends existing shared-ownership patterns. The +90 MB
peak (Arc headers on AST nodes) is flagged: candidates to recover it are
`Arc<[T]>` for the three list variants (drops the inner Vec header) and
earlier dependency-AST release.

## Final measured distribution

4-way interleave (jobs=1) at the `8161a72`/`b9d1334` state: OLD-sys
15.17–15.99, NEW-sys 13.94–14.74, OLD-mim 11.84–11.87, NEW-mim 10.93–11.45.
Property-interning increment (`a2c7247`), 5-run interleave vs `b9d1334`
(jobs=1, one window): OLD 15.40/14.67/14.70/14.68/14.44 (median 14.68),
NEW 14.87/14.69/14.58/14.53/13.91 (median 14.58); user OLD median 12.66,
NEW median 12.55. All runs (every commit, both allocators, jobs=1/auto)
sha256 `4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59`.
Raw log: `TRPC-ALLOCATION-VOLUME-MATRIX.txt`.

## Validation (final commit `a2c7247`)

```text
cargo fmt --check ✓   cargo check --workspace ✓ (0 errors)
cargo nextest run --workspace: 1546/1546
pnpm oracle:test: green    oracle sweep: 97/97, messageDriftOnly=5 (pre-existing)
real projects: 28 pass / 0 fail / 3 conditional skips
raw cmp vs b9d1334 binary: tRPC ✓ zod ✓ ky ✓ ofetch ✓ (jobs=1 and auto)
memory: peak fp ~1.97 GB (≤2.00 ✓), finish fp ~464 MB (≤0.70 ✓)
```

Final commit: `a2c7247` (`git log --oneline` from 5515b68 shows the four
retained commits: `5d05b06` census, `beb33d5` ParsedType Arc, `8161a72`
small-alloc batch, `a2c7247` PropertyMap interning).
