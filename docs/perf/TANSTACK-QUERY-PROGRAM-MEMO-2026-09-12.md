# tanstack-query — the program-lifetime interface memo (2026-09-12)

> **Point-in-time measurement report.** Every number below was measured on
> 2026-09-12 on a dirty working tree (`6e034fd` plus the uncommitted checker
> work of the preceding days) with the machine under external load; wall
> clocks are therefore not reported, instructions retired and peak physical
> footprint are. It does not describe current behavior. Current gate values
> live in [CURRENT_STATUS.md](../../CURRENT_STATUS.md).

Follows [TANSTACK-QUERY-PROFILE-2026-09-10.md](TANSTACK-QUERY-PROFILE-2026-09-10.md).

## Result

| corpus | before (instr / peak footprint) | after | diagnostics |
| --- | ---: | ---: | --- |
| tanstack-query | 40.0G / 1030 MB | **6.0G / 189 MB** | byte-identical (30 → 28, two false positives closed) |
| trpc | 64.6G / 1267 MB | 57.7G / 885 MB | byte-identical |
| zod | 27.2G / 409 MB | 23.9G / 371 MB | byte-identical |
| ts-pattern | 2.16G / 64 MB | 2.20G / 65 MB | byte-identical |
| ofetch | 1.25G / 71 MB | 1.28G / 73 MB | byte-identical |
| ky | 0.71G / 35 MB | 0.70G / 37 MB | byte-identical |

"before" is the tree's binary at the start of the day; "after" is the same
tree with the changes below. `interface_resolution_attempt_count` on
tanstack-query went 650,063 → 22,483; `interface_member_declaration_visit_count`
4.77M → 72k. The workspace suite (1867) and `oracle:sweep --all` (206 presets,
0 mismatches) are unchanged.

`pnpm run bench:tanstack-query` afterwards, 5 runs interleaved under a load
average of 67 (so a ratio, not a number to record): surge-ts 0.64s median /
190 MB against tsgo 0.64s / 491 MB, tsgo single-threaded 1.15s / 389 MB and
tsc 4.68s / 680 MB.

## What landed

1. **Relative `declare module "./x"` augmentations merge into the target
   file's own declaration table** (`binding.rs`, P1), not only into the
   export-table copy each importer receives. Heritage resolves a base under
   the declaring file's scope, so `Identifier extends BaseNode` never saw the
   `parent` hung on `BaseNode` — and which consumer triggered an expansion
   decided what a shared expansion contained. The importer-side application
   now adds only values and brand-new declarations. Fixture:
   `relative-module-augmentation-heritage-basic`.
2. **The program-lifetime interface memo is on by default**
   (`SURGE_IFACE_MEMO_PROGRAM=0` opts out) with the per-module table instance
   id dropped from its fingerprint where the body provably does not read the
   consumer's table (`SURGE_IFACE_MEMO_TABLE=1` restores it). Both knobs
   existed as sealed experiments; enabling them moved two diagnostics, and
   each move was a real defect:
   - the augmentation gap above;
   - **a body expanded under a shadow context was stored.** The shadow's
     environment store dies with the shadow, so every later peel of a lazy
     reference captured there returned `Unknown` — `Variable` lost its
     inherited `scope`. `DeclarationEnvironmentStore::is_program_lifetime`
     now gates the store.
3. **Self- and member-level cycles no longer keep a body out of the memo.**
   The gate required a body to touch no cycle machinery at all, which
   excluded every mutually recursive user cluster — `QueryObserverBaseResult`
   was re-expanded 91k times under 18 distinct keys with zero stores. A member
   annotation that re-enters an in-progress frame embeds a cycle reference
   carrying its own resolved arguments, a nominal handle any reader peels on
   demand. What a re-entry does poison is **heritage**: a base that is
   mid-resolution contributes no inherited members. The memo now refuses only
   a heritage re-entry into an outer frame
   (`LAST_EXPANSION_HERITAGE_LOWEST_CYCLE`). Storing outer-cycle bodies
   unconditionally exposed `QueryObserverOptions` spread without `queryKey`;
   the heritage gate closes it.
4. **`{ ...anyValue, k: v }` is `any`**, as in tsc. Surge skipped a
   non-object spread source and closed the literal over the remaining
   properties, which reported every required member as missing once
   `PersistedQueryClientRestoreOptions` resolved cleanly instead of degrading
   open. A spread of surge's own degradation sentinel keeps the literal open.
   Fixture: `object-spread-any-source-basic`.

## Follow-ups landed the same day

Measured on the loaded machine only (load 60–100), so by instructions
retired on tanstack-query, six corpora byte-identical throughout:

5. **Export-collection shadow skips annotated arrow bodies.** An
   initializer's arrow with a written return type has its signature fixed by
   annotations, and the shadow context discards diagnostics, so its body
   check there was pure cost (`skip_annotated_function_bodies`). −5.5%.
6. **The analysis-round value seed runs only where signature collection can
   read a value**: a file containing `typeof`, or a class whose heritage
   names a value. 177 → 57 seeded files. −8%.
7. **`export =` alias adoption shares declaration handles** instead of
   deep-cloning every entry of the target namespace into a first-wins insert
   (`typescript.d.ts` for every `import * as ts` consumer).
8. **`fast_process_exit`**: the CLI exits right after rendering, and the
   checker skips the end-of-run teardown when nothing observes it (off under
   any RSS/timing/census instrumentation and for library callers).

## Method notes

- Every A/B was two arms of one binary (env gates) or two snapshot binaries
  built from the same tree state, compared by instructions retired and by
  byte-equality of `--format json` output on six corpora. Wall clock was
  unusable all day (load 8–25).
- `SURGE_PROGRAM_MEMO_DUMP=1` (new) prints one line per program-memo miss,
  store and skip with the skip reason; `program_memo_{hit,miss,store}_count`
  join the extended diagnostics. The 91k-misses / 0-stores signature was
  what pointed at the cycle gate.
- The dead-environment peels (`checker_context()` → `None`) also occur
  without the memo — 1,561 per tanstack run — and are by design for source
  files (see the note at `values.rs` on the shadow store). They are a
  correctness surface worth its own look.

## Where the time goes now (sample, 1487 samples)

| region | share |
| --- | ---: |
| `collect_exportable_value_symbols` (module analysis, seed + export table) | 37% |
| … of which `check_arrow_function_expression_anchored` under initializers | 27% |
| `collect_exportable_value_symbols` (local-values pass + check-phase validation) | 12% |
| `check_program_file` statement checking | ~25% |
| `collect_function_signatures_from_statements` (analysis) | 5% |
| `validate_local_type_declarations` | 2% |
| check-phase signature pre-pass | 1% |

The exportable-value collection evaluates every exported initializer —
arrow bodies included — in the analysis seed, again in
`build_module_export_table`, again in the local-values pass and once more as
check-phase validation symbols, before the check phase checks the body
itself. `SURGE_ANALYZE_SPLIT`: round 2 `values=0.295s`, `et_values=0.379s`.
That is the next lever; the check-phase re-pass and the placeholder-argument
cache key that the 2026-09-11 roadmap listed are now marginal (2% and 1%).
