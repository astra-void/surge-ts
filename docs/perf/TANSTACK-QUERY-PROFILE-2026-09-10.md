# tanstack-query profile — where the 4.4s goes (2026-09-10)

> **Point-in-time measurement report.** Every number below was measured on
> 2026-09-10 on a dirty working tree (commit `6e034fd` plus the uncommitted
> checker work of that day) with the machine under external load; it does not
> describe current behavior. Current gate values live in
> [CURRENT_STATUS.md](../../CURRENT_STATUS.md).

Inputs: `.bench/real-projects/tanstack-query/` (bench `jobs1.json`,
`timings.txt`, `measurement.md`), `SURGE_STAGE_TIMES`, `SURGE_MODULE_TIME_DUMP`,
`SURGE_TRACE_HAD_ERROR`, `SURGE_ANALYZE_SPLIT`, and macOS `sample` call-graph
profiles of the release binary.

## Headline numbers (bench, jobs=1, 5 runs, `6e034fd` + dirty tree at 10:26)

| tool | median wall | peak footprint |
| --- | --- | --- |
| tsgo | 0.63s | 519MB |
| tsgo single-threaded | 1.22s | 403MB |
| surge-ts | 4.40s | 1.09GB |
| tsc | 5.90s | 689MB |

`--jobs auto` is not slower than `--jobs 1`: the bench's 5.8s auto median came
from load noise (three interleaved pairs: user 4.85–5.26s vs 4.95–5.15s).

## Where the time goes (stage times, jobs=1)

| stage | wall | note |
| --- | --- | --- |
| frontend + parsing + ambient/global | 0.2s | |
| preliminary module analysis | 0.07s | thin; real per-module time 21ms total |
| **final module analysis** | **2.9s** | 542 modules; real per-module time 2.83s |
| module binding + local values | 0.15s | |
| **check phase** | **1.7s** | |
| finish (teardown) | 0.3s | |

The final analysis round is concentrated in ~12 tiny source files:
`infiniteQueryBehavior.ts` 345ms, `useQueries.ts` 298ms, `streamedQuery.ts`
280ms, `mutationOptions.ts` 270ms, `useMutation.ts` 248ms,
`__tests__/utils.ts` 246ms, `usePrefetchInfiniteQuery.tsx` 160ms, … (top 12 ≈
2.3s of 2.83s). `SURGE_ANALYZE_SPLIT` attributes essentially all of it to the
**signature** split (`signatures=0.677s` of a 0.68s round on a one-file probe);
values and export tables are ~1ms.

## Mechanism (profile, 4394 samples)

- 47% of all samples: `analyze_module` → `collect_function_signatures_from_statements`
  → `collect_function_declaration_signature` → `map_function_signature` →
  `resolve_parsed_type` → `resolve_named_type` → `resolve_interface` … recursing
  through `resolve_function_type` / `resolve_type_alias` /
  `resolve_conditional_type` / `merge_intersection_members_now`.
- A further 12% is the *same* pre-pass run again by `check_program_file`, and
  `check_function_declaration` maps each signature a third time.
- Self time is allocation and comparison churn: `mi_free` 300, `memmove` 296,
  `memcmp` 262, `mi_malloc_aligned` 205, `SipHash::write` 149, `IndexMap::insert`
  86, `union_type` 71, `default_lib_name_flags` 67, `Type::clone` 61,
  `ParsedType::clone` 54, `declaration_environment` 43.

Why the expansions repeat instead of hitting a cache:

1. A source function's signature pre-pass (`build_type_parameter_substitution`)
   binds its type parameters to **placeholders whose value is `Type::Unknown`**.
   `QueryBehavior<TQueryFnData, TError, InfiniteData<TData, TPageParam>>` is
   therefore a user interface instantiated with `unknown` arguments.
2. Every interning tier rejects it: the concrete tier needs no open
   type-parameter scope (`map_function_signature` pushes one), the
   signature-context tier excludes placeholder and `unknown` arguments and the
   analysis phase, the library tiers need a library-scoped declaration.
   Counters: `interface_resolution_attempt_count` 655k,
   `interface_method_mapping_attempt_count` 613k against
   `unique_interface_method_instantiation_count` 1,255;
   `interface_member_declaration_visit_count` 4.9M for 588 unique member
   declarations; `function_type_payload_alloc_substitution_changed_count`
   465k; `union_type_copy_from_substitution_changed_count` 1.0M.
3. Each expansion walks the whole reachable query-core graph
   (`QueryBehavior` → `FetchContext` → `Query` → `QueryCache` → `QueryClient` →
   `QueryObserver` → …) structurally, and many of those expansions are
   **degraded** (`interface_resolution_degraded_count` 48k), so the peel pins
   and the interner would not keep them even where eligible. Degradation
   origins (`SURGE_TRACE_HAD_ERROR`): 7.8k `lookup-miss` (almost all in the
   map-less value/export-table phases, `map_len=0`, plus `@types/node`
   cross-`declare module` names and `TSESTree.*`), 1.1k `alias-cycle
   'TuplePrefixes'` (a recursive *conditional* alias surge flags as an illegal
   cycle; tsc accepts it), 243 `infer-unbound`.

## Experiments

### A. Defer placeholder-argument user interface instantiations (kept, opt-in)

`SURGE_DEFER_PLACEHOLDER_INSTANTIATIONS=1` turns `Foo<T, …>` (with `T` a
signature placeholder) into a lazy nominal reference keyed under
`DeclarationNamespace::PlaceholderInstantiation` + the placeholder argument
mask (`resolve/named.rs`).

- Only 1,872 deferrals fire; the deferred references are peeled straight back
  by `merge_intersection_members_now` (`WithRequired<…> = T & {…}` is a mixed
  reference/object intersection and merges eagerly), by `resolve_conditional_type`
  (the `extends` pattern is resolved before the check type is found to be
  undecidable) and by heritage resolution. Interface resolution attempts
  went *up* 6% (678k → 722k); member visits +7%.
- Diagnostics: ky/ofetch/unnamed byte-identical; trpc −11 (all removals);
  zod −7 +4 (`ZodLazy<T>` nominal fallback of a call whose inference was
  skipped); tanstack +8 vs the 112 baseline (`queryClient.ts` TS2345/TS2339 —
  latent false positives that degraded, open shapes used to hide; the TS2345
  reproduces on the unmodified tree with a caller whose type parameter is
  `TQueryKey extends QueryKey = QueryKey`).
- Time: tanstack user 5.65/6.53/6.72s → 5.37/5.72/5.88s (3 pairs, load 30–50,
  not conclusive); zod user neutral, **zod peak RSS 545MB → 782MB** (the
  captured environments outlive the pre-pass).

Verdict: not a landing on its own. It is the first half of the real lever and
is left in the tree gated off.

### B. Treat recursive conditional aliases as legal recursion (reverted)

`alias_body_supports_recursion` returning true for `ParsedType::Conditional`
(tsc semantics since 4.1) halves `interface_resolution_degraded_count`
(48k → 26k) but leaves attempts/member visits unchanged, and unmasks 11
tanstack false positives (`queryClient.ts` TS2339 ×5 on spread results missing
members inherited through `extends WithRequired<QueryOptions<…>, 'queryKey'>`,
`queryObserver.ts` TS2345 ×2 / TS2367 ×2, TS2741 ×2). Reverted; land it after
the heritage-through-alias-intersection gap is fixed.

### Noted, not attempted

- `declaration_environment()` is called per lazy-reference creation (534k
  creations, ~5% of analysis samples in the deferral run).
- `generic_instantiation_display_name` formats a `String` per generic
  instantiation (`format_inner` 182 samples).
- `finish` (teardown drops) is 0.3s of a 5.3s run; a CLI fast-exit would skip
  it but must not bypass `clear_program_type_caches` semantics for library use.
- The working tree's uncommitted `enforce_inferred_constraints`
  (`checks/call/instantiate.rs`) moved the tanstack aggregate from 112 to 225
  diagnostics (+115 TS2345 in `*.test-d.tsx`); the 10:26 bench (120) predates
  that edit. **No longer reproducible as of 2026-09-10 14:00** (dirty working
  tree, so treat the counts as indicative, not as a recorded gate result): with
  the guards the function now carries, it performs 0 candidate replacements
  across the whole aggregate — 522 constrained type parameters reach the check
  and 501 of them skip because inference produced no candidate at all, leaving
  the parameter at its `Unknown` placeholder. Disabling every guard raises that
  to 1 replacement and changes no diagnostic; the enforcement call and a
  `return`-immediately stub emit byte-identical output. The +115 therefore needs
  inference to start yielding concrete candidates for constrained parameters —
  which is exactly what "The lever" below is trying to achieve, so the cascade
  is dormant rather than fixed. See the `constraint_operand_is_settled` /
  `had_error` gate added alongside this note for what keeps a degraded or
  unsettled constraint from being enforced when that day comes.

## The lever

tsc never expands `QueryBehavior<…>` structurally while collecting a signature:
a generic instantiation stays a nominal type whose members resolve on demand
and are cached on the type object. surge's equivalent needs three things
together, each measured here to be insufficient alone:

1. nominal deferral of placeholder-argument user instantiations (A);
2. lazy consumers — mixed reference/object intersections deferred like the
   all-reference case (`intersection.rs`), conditional types that skip the
   `extends` resolution when the check type is an undecidable placeholder, and
   heritage bases kept nominal until a member is read;
3. an interning key that admits placeholder arguments (mask) and the final
   analysis round (environment generation in the key), so the one expansion
   that does happen is shared by the ~12 files that repeat it.

Without 2, A only moves the expansion; without 3, 1+2 still expand once per
site; without B (or the equivalent de-tainting of `lookup-miss` in the
map-less phases) the expansions stay degraded and uncacheable.
