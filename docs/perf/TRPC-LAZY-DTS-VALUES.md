# tRPC: lazy library value annotations — the shadow-environment capture design

Branch `perf/lazy-dts-values`. Second attempt at the final-round value-
collection lever ([TRPC-THIN-PRELIMINARY-VALUES.md](TRPC-THIN-PRELIMINARY-VALUES.md)
records the first attempt's both-ways drift and the decision to require a
"full shadow-environment capture" design before retrying). This session
root-caused that drift to three specific, fixable mechanisms — none of which
was the feared broad "force-time environment ≠ shadow environment" problem.

## What changed

`declare const x: T` in a library `.d.ts` gets a `LazyDeclarationAnnotation`
reference (the same machinery as the default-on lazy dependency function
signatures, `signature_component: None`) instead of an eagerly mapped type,
in every exportable-value collection round. The annotation maps on first
read under the captured declaration environment.

## The three root causes of attempt #1's drift

1. **The shadow's environment store dies with the shadow.** Value collection
   runs in a per-file shadow `CheckerContext`; `new_with_shared_options`
   creates a fresh `DeclarationEnvironmentStore`, and environment handles
   hold `Weak<Store>`. Every force after the shadow dropped hit
   `checker_context() → None → Unknown` — silently. The playwright
   `TestType` TS2339s were member lookups on that `Unknown` (via
   `get_property_access_type`'s reference arm, which resolves and then
   reports a missing member on the resolved type — the Unknown-suppression
   discipline never sees it). Fix: when lazy values are enabled, the shadow
   shares the caller's persistent store. This was 97 of the 97 diff lines'
   upstream cause.
2. **`typeof` queries resolve against collection-time working symbols.**
   Environment capture deliberately drops the working value table (the
   memory-lifetime rules), and `module_local_values_by_file` is captured
   *before* it is populated (that happens after the final round). A deferred
   `typeof stringType` therefore degrades at force time (zod3's renamed
   exports: TS2304 `ParseInput` inside the recovered mapping). Fix: a
   recursive filter keeps any typeof-bearing annotation eager. Primitives,
   literals, and keyword types also stay eager — deferring them is free of
   benefit and exposes unforced references to structural checks (the
   `skipToken: unique symbol` TS2367).
3. **Variant matches that don't peel.** `callable_property_signature`
   unwrapped references peeling to *objects* but returned the original
   reference for ones peeling to *functions*, so the caller's match fell to
   the TS2349 arm. Fixed to surface the peeled function as the callable —
   a latent robustness gap, not lazy-specific.

A fourth design element prevents a drift class that never got to manifest:
the resolver returns the mapped type **unpeeled** (matching eager
`map_parsed_type` output). An eager peel would bake a recovered-environment
expansion snapshot into the symbol; unpeeled, nested named references expand
through the live-context peel path exactly as the eager shape does.

## Results

- tRPC: **byte-identical** (full diff, 2190 diagnostics), and interleaved
  wall (jobs=auto) base 8.8–9.5s → lazy 7.9–8.3s, every round pairwise
  ≈ −1.0s.
- ky/ofetch: byte-identical × jobs auto/8/1.
- zod: exactly **one diagnostic removed** — `zod3-string.ts` TS2339
  "Property 'parse' does not exist on type 'ZodString'", the documented
  degraded-resolution false-positive class (tsc-clean; `ZodString` has
  `parse`). Force-time resolution at check phase is richer than the map-less
  eager collection, so the FP disappears. Adjudicated as an improvement.
  Deterministic across runs and jobs (single stable hash).
- Combined with the thin-preliminary change, the session total on this
  machine: tRPC ~10.5s → ~8.1s.

## Gates

- Byte matrices above; determinism ×3 runs × 3 job counts.
- `cargo nextest run --workspace`, oracle sweep `--all` — see session log.
- Default ON; `SURGE_LAZY_DTS_VALUES=0` opts out (measurement escape hatch).

## Follow-ups

- `SURGE_LAZY_VALUE_TRACE=<substr>` probe (create/force sites, OnceLock'd
  filter) stays for future hunts.
- The typeof-eager restriction can potentially be lifted by re-capturing or
  falling back to live `module_local_values_by_file` at force time — only
  worth it if profiling shows typeof-bearing annotations carry real cost.
- The `unnamed` corpus is not in this checkout; its FP suite should be
  re-run before fully trusting the lazy rounds there (same caveat as the
  thin-preliminary change).
