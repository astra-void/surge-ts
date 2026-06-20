# Iterator-Type Modelling & Recursive-Resolution Stability (design)

**Status: DESIGN / track open.** Dedicated follow-up for the last open
suppressed-diagnostics item (#8): the suppressed `TS2304 'BuiltinIteratorReturn'`
lib note and, behind it, surge's iterator/`Set`/`Map` typing. This document
captures the debugged root cause and a phased plan; no code in this track is
landed yet (every partial attempt is reverted — see "Attempts" below).

## Goal

Let `type BuiltinIteratorReturn = intrinsic` (and the `intrinsic`-bodied lib
aliases generally) **resolve** instead of being dropped to `TS2304`, *without*
regressing any gate (ky 0/0, oracle sweep 76/76, zod/trpc no new FPs). The
surface diagnostic is cosmetic (suppressed, native-profile-only); the real value
is correct `Set`/`Map`/array-iterator member typing once the alias resolves.

## Why it is not a point fix — the blocker (gap 2)

Mapping `intrinsic ⇒ unknown`/`any` makes the alias resolve, but it deepens the
type graph that `Set`/`Map`/array iterators (and thus every consumer) touch, and
that **breaks ky 0/0** with one residual error:

```
source/core/Ky.ts(145,37): TS2339: Property 'length' does not exist on type 'unknown'
```

from `const initHooks = options.hooks?.init ?? []; initHooks.length`.

### Debugged trace (intrinsic applied)

1. `??` — `unknown ?? right` returns `unknown` (`infer/expression/mod.rs`,
   `checks/expr.rs`). So an `unknown` left propagates to `.length`.
2. the `unknown` left is `options.hooks?.init`:
   `infer_optional_property_access` returns `Unknown` whenever its object infers
   to `Unknown`.
3. the object `options.hooks` infers to `unknown` — **even though `Options`
   itself resolves to a real object** (`options.fakeTopLevelXYZ` correctly reports
   `TS2339 … on type 'Options'`). So it is the **`hooks` property's type** that is
   `unknown`: when `Options`'s body is resolved, the field `hooks: Hooks` hits a
   recursive **cycle edge** that degrades to `unknown`.

The behaviour is **deterministic** (identical across runs), not random — it is
order/cache-dependent: `intrinsic` changes the order in which the mutually
recursive `Options`/`Hooks`/`InitHook` cluster is resolved, so the `hooks` field
reaches a cycle edge that does **not** get the #1 lazy-reference treatment.

### Root-cause model — corrected by Phase-1 instrumentation

Initial hypothesis was one of three cycle-edge paths (in-frame — fixed by #1;
cross-frame in `get_cached_named_type_resolution`; lazy-peel-limit in
`LazyInstantiation`). **Phase 1 instrumented all three and none fire** for the
failing `Options.hooks → Hooks` edge, and `Hooks` is **not** cached as
`Resolved{unknown}` either. So the `unknown` is none of those.

What Phase 1 *did* establish (debug, intrinsic applied):
- `options.hooks` infers to `unknown | undefined` while `Options` itself resolves
  to a real object — so it is the **`hooks` field type** that is `unknown`.
- `Hooks` resolves fine in most contexts (a real `{ init?: …; beforeRequest?: … }`
  object, 15×), but **some** of its resolutions carry `had_error = true` and a
  *degraded* body, e.g. `init?: ((unknown) => void)[]` — i.e. `InitHook`'s
  `(options: Options) => void` resolved with its `Options` parameter degraded to
  `unknown`.
- So the real mechanism is **`had_error` propagation through the recursive
  strongly-connected component** `Options → Hooks → InitHook → Options`: an inner
  `Options` edge degrades to `unknown`/`had_error` (via a path *not* covered by the
  three above — it eluded targeted instrumentation), and that `had_error`
  propagates up so the enclosing `hooks` field is stored as `unknown`.

`intrinsic` does not create this — it perturbs the cluster's resolution *order* so
the degraded variant is the one that lands in `Options`'s `hooks` field. It is a
**resolution-stability** property of the whole SCC, not a single mishandled edge,
which is why no point fix (deferred reference, depth bump) cleared it.

## Attempts (all reverted — empirically ruled out)

- `intrinsic ⇒ any` and `intrinsic ⇒ unknown`: identical ky breakage.
- Raising `MAX_LAZY_PEEL_DEPTH` (24→64) and `MAX_SAME_DECLARATION_PEELS` (3→8):
  **did not clear gap 2** → not the peel-depth bound.
- Returning a deferred reference from the cross-frame `get_cached` path (instead
  of `unknown`): **did not clear gap 2** either — so either the failing edge is the
  peel-limit path, or the deferred reference re-peels to `unknown`.
- Minimal `Opts/Hooks/Init` triple: **does not reproduce** → specific to ky's
  large cluster (size/shape dependent).

Two genuinely-correct *secondary* fixes were also prototyped and shelved (inert
or churn-only without the iterator work): the constrained-indexed-access
suppression (`Required<Hooks>[K]`, gap 1) and `unknown ?? right` handling.

## Candidate approaches

1. **Unify cycle-edge handling (preferred).** Make the cross-frame *and*
   peel-limit edges return the same lazy nominal self-reference #1 returns
   in-frame, so a recursive field is never `unknown` regardless of resolution
   order. Risk: a deferred reference that re-peels into the same bounded path can
   loop or re-degrade; needs a peel guard that yields the *reference* (not
   `unknown`) at the bound. This is the core of the track.
2. **Don't deepen the graph.** Resolve `intrinsic` aliases to a non-expanding
   nominal type so `Set`/`Map`/array iterators don't pull more graph. Cheaper but
   leaves iterator member typing degraded (only removes the cosmetic TS2304).
3. **Order-independence.** Make the recursive cluster resolve to the same fixed
   point regardless of entry order (memoize the whole strongly-connected component
   together). Most correct, largest.

### Phase 2/3 attempts (this session) — all ineffective or regressive

Three targeted patches of the existing architecture were tried with the full
verification suite as the safety net; none works:

- **cross-frame edge → deferred reference** (`get_cached_named_type_resolution`
  returns a lazy ref instead of `unknown`): did not clear gap 2 (the failing edge
  is not the cross-frame path — consistent with Phase 1).
- **raise the lazy-peel bounds** (24→64, 3→8): no effect.
- **defer any `had_error` user-type resolution to a lazy reference** (don't cache
  the degraded structure): **regressed badly** — `Options` then peeled to only its
  inherited `RequestInit` shape, *losing* ky's own fields (`baseUrl`, `hooks`,
  `retry`, `onDownloadProgress`, …), so ky gained ~10 new errors. The deferred
  reference re-peels to a degraded `Options`, i.e. the instability is in how the
  cluster *merges extends + own members under recursion*, not just whether a single
  result is cached.

**Conclusion:** the existing recursive-resolution machinery cannot be bent into a
fix by a targeted patch — every lever either misses or regresses. Closing #8 needs
approach 3 done properly: compute the whole SCC's fixed point (collect the cluster,
resolve members against the cluster's own deferred references, merge `extends` +
own members once, memoize the component as a unit) rather than resolving each
declaration independently with order-dependent `had_error` propagation. That is a
ground-up redesign of the recursive cluster resolver — a major architectural
change, scoped as its own project, not a session patch.

## Phased plan (each phase gated: ky 0/0 + sweep 76/76 + zod/trpc no new FP)

1. **Characterise the failing edge — DONE (Phase 1).** Result above: it is *not*
   any of the three cycle-edge paths; it is `had_error` propagation through the
   `Options/Hooks/InitHook` SCC, order-perturbed by `intrinsic`. This redirects the
   work from "fix one edge" (approaches 1/2) to "stabilise the SCC" (approach 3).
2. **Locate the inner `Options → unknown` edge** that seeds the `had_error`. The
   targeted prints (cross-frame / peel-limit / cached-Resolved) all missed it, so
   the next step is to instrument every `ResolvedType { had_error: true }`
   *origin* for the cluster names and find which one fires first. (No behaviour
   change.)
3. **Stop the degradation propagating.** Either (a) make that origin return a lazy
   self-reference like #1 instead of `unknown`/`had_error`, or (b) resolve the SCC
   to a single shared fixed point so a member is never stored degraded
   (approach 3). Verify gap 2 clears in isolation.
4. **Land `intrinsic`** + the gap-1 indexed-access suppression + `unknown ??`
   handling together; full sweep + reals.
5. **Iterator member typing** — only if value warrants: model
   `IteratorObject`/`SetIterator`/`Set`/`Map` so `for…of` and iterator results
   type correctly, not just avoid the cascade.

**Assessment after Phase 1:** the dominant blocker is SCC resolution stability
(approach 3), a systematic change to how the recursive cluster memoises and
propagates `had_error` — sizeable and high-blast-radius. Phases 2–3 are the next
concrete steps; they remain investigation-heavy with no guaranteed quick
convergence.

## Out of scope / non-goals

Full `Symbol.iterator` protocol typing and generic iterator inference are not
required to close #8; phases 1–3 (stop the cascade, resolve the alias) are.
