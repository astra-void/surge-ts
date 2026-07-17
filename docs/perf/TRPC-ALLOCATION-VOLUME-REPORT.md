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

## Remaining allocation owners (post-change profile)

```text
malloc self-time ≈ 17.5% (from 22–24%)
fingerprint_type SipHash          181 self samples (semantically pinned)
interface member resolution        76 malloc-parent samples (property-map +
                                   member Strings; needs PropertyNameId
                                   interning in surge-ts-types to go further)
union_type / FunctionType::new /
TypeReference::new                 ~85 (pre-intern Type-layer temporaries)
display strings (Type::name,
parsed_type_display, join)         ~64 (byte-exactness-sensitive)
ParsedType::clone self             108 (refcount atomics + census branch, 33M events)
memmove                            550 (parser + Vec copies)
```

Next-step recommendation: (1) intern property names (`PropertyNameId` /
`Arc<str>` keys in `PropertyMap`) — the largest single remaining owner; (2)
reduce clone *events* by threading `Arc<ParsedNamedType>` deeper into the
lazy-reference captures; (3) then re-evaluate speculative transactional
checking (see TRPC-5S-REPORT.md) for the parallel win — allocation work has
now consumed most of the serial headroom.

## Memory safety / lifetime notes

No new arenas, no reset semantics, no Drop-skipping structures were added;
the Arc conversion only extends existing shared-ownership patterns. The +90 MB
peak (Arc headers on AST nodes) is flagged: candidates to recover it are
`Arc<[T]>` for the three list variants (drops the inner Vec header) and
earlier dependency-AST release.

## Final measured distribution

4-way interleave (jobs=1): see the outcome table; per-run spread OLD-sys
15.17–15.99, NEW-sys 13.94–14.74, OLD-mim 11.84–11.87, NEW-mim 10.93–11.45.
A separate 5-run jobs=auto matrix at the final commit: 14.07–16.02
(median 15.13, hot window), byte-identical to jobs=1. All runs sha256
`4d69a2d5f549616083afa9c9e3bccc3484a8bdc96457988fd1f060b805b5ee59`.
Raw log: `TRPC-ALLOCATION-VOLUME-MATRIX.txt`.

## Validation

```text
cargo fmt --check ✓   cargo check --workspace ✓ (0 errors)
cargo nextest run --workspace: 1546/1546
pnpm oracle:test: green    oracle sweep: 97/97, messageDriftOnly=5 (pre-existing)
real projects: 28 pass / 0 fail / 3 conditional skips
raw cmp vs 5515b68 binary: tRPC ✓ zod ✓ ky ✓ ofetch ✓ (jobs=1 and auto)
```

Final commit: recorded in the branch log (`git log --oneline` from 5515b68).
