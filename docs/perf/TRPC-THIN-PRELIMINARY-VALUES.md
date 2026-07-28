# tRPC: thin exportable-value collection — 10.5s → 8.2s (−21%)

Branch `perf/thin-preliminary-values`. The session started from the
5-second-goal arithmetic: frontend ~1.4s is squeezed, the parallel paths
(check, analysis) are all measured at wall parity, so the only route runs
through the serial analysis stages — preliminary 3.5s + final 2.8s +
local_values 1.0s of a ~10.3s checking bucket.

## How the target was found (probe trail)

1. `SURGE_EQ_STATS` (pre-existing probe, first run this session): the
   input-equality final-round skip is dead — 74.3% of files are output-equal
   across rounds but carry only 35.2% of final-round time, the sound
   predictor captures 2.5% (0.06s), and 268 of its predictions are unsound.
2. `SURGE_MODULE_TIME_DUMP`: per-file analysis cost is diffuse (top file
   116ms, 50% of time in the top 54 files) — no single-file hotspot, and
   content-duplicate files (heyapi fixtures, dist `.d.cts`/`.d.mts` pairs)
   account for only ~0.34s per round.
3. New `SURGE_ANALYZE_SPLIT` probe, splitting `analyze_module`: **value
   collection dominates, not signature collection.** Per round:
   `values≈0.28s signatures≈0.39s export_table≈1.30s`, and the export-table
   time is almost entirely its internal *second* `collect_exportable_value_
   symbols` call (`et_values≈1.3-1.8s`, `et_statements≈0.03s`).
4. macOS `sample` profile under `collect_exportable_value_symbols`: 96% in
   `check_variable_declaration_with_symbols`, of which ~75% is
   `map_parsed_type_with_substitution` (eager annotation resolution) and ~20%
   `evaluate_expression` (initializer inference). AST clones and shadow-
   context churn are noise. (The 13-day-old "94% of peels are signature
   collection" backtrace predates the 31s→11s era and no longer describes
   the cost structure.)

The structural finding: `build_module_export_table` runs **three times per
module file** — the round-0 type-binding pass (`collect_preliminary_module_
type_bindings`), the preliminary analysis round, and the final analysis
round — and each run eagerly resolves every declared value's annotation.
Only the final round's output survives into anything the check phase reads.

## The change

Superseded rounds (round 0 + preliminary) collect exportable values THIN:

- identical symbol **name** surface (variables present with their let/const/
  var kind, namespace value objects keep their permissive member sets),
- `Type::Unknown` instead of a resolved annotation/initializer type,
- no shadow `CheckerContext`, no `check_variable_declaration` at all.

The final round is untouched (full fidelity), as are all other
`build_module_export_table` call sites (ambient modules, namespace surfaces,
validation paths). `SURGE_THIN_PRELIM=0` restores the eager behavior.

## Why it is sound

The check phase never consumes intermediate-round value types: it reads the
final round's export tables (rebuilt with full fidelity) and
`module_local_values_by_file` (populated after the final round). The
preliminary value types only ever bootstrapped the *name* surface for import
binding (which needs names and declaration kinds, not resolved types), and
base preliminary values were already low-fidelity (resolved with empty or
v0 imports). Empirically: byte-identical diagnostics on trpc (2190), zod,
ky, ofetch × jobs auto/8/1, thin vs eager.

## Measurements (tRPC)

Analysis-stage split (`SURGE_ANALYZE_SPLIT`, jobs=auto):

```
                 values     signatures   export_table (et_values)
prelim  eager    0.27s      0.41s        1.30s (≈1.8s incl. round-0)
prelim  thin     0.001s     0.39s        0.03s (0.01s)
final   (both)   0.28s      0.36s        1.33s (1.30s)
```

Wall (interleaved, 4 rounds, jobs=auto): **base 10.30–10.78s, thin
8.16–8.49s** — every pairwise round ≈ −2.2s.

## Gates

- Byte-identity: 4 corpora × 3 job configs, thin vs eager — 12/12; default-on
  vs `SURGE_THIN_PRELIM=0` — 12/12; all hashes equal historical baselines.
- `cargo nextest run --workspace`, oracle sweep `--all` — see session log.

## Follow-ups

- The remaining analysis cost is now the FINAL round's value collection
  (~1.6s incl. et_values). **Laziness was tried the same day and reverted:**
  extending `LazyDeclarationAnnotation` to library `declare const x: T`
  annotations drifts tRPC by ~50 diagnostics in both directions (playwright
  `TestType` members lost → new TS2339; node `path` values resolving *better*
  than the eager path's degraded `{}` → diagnostics vanish). Force-time
  resolution runs under the captured declaration environment, which is not
  the collection-time shadow environment (`module_value_fallback`, scope
  specifics). A byte-stable version needs full shadow-environment capture.
- The `unnamed` corpus (ComponentProps/typeof-heavy) is not in this checkout;
  its FP suite should be re-run before assuming the thin rounds are safe for
  it (the values.rs comment history shows that corpus is the sensitive one).
- `signatures` (~0.39s/round) still runs full in the preliminary round; its
  consumers overlap with value collection's and may tolerate the same
  treatment — separate experiment.
