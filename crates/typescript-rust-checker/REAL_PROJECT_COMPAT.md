# Real Project Compatibility

This crate tracks compatibility in narrow, oracle-backed phases rather than claiming full TypeScript parity.
Its compatibility surfaces are raw measurements, not root-cause classifiers.

## Current coverage

- v1.2.5 is a performance pass after v1.2.4, not a new TypeScript semantic
  phase. No new TypeScript surface was added; exact diagnostics remain 0 and raw
  oracle match stays yes. Four changes land. (1) Path canonicalization is
  memoized per run in both `typescript-rust-checker` (`paths.rs`,
  `canonicalize_if_exists_string`) and `typescript-rust-config` (`paths.rs`,
  `canonicalize_if_exists`), so the repeated `std::fs::canonicalize` (realpath)
  syscalls in type/module resolution and the project-discovery import-graph
  fixpoint are paid once per distinct path instead of on every probe; the caches
  are thread-local and cleared at the start of each check. (2) The
  instrumentation counters that funnel through one global
  `Mutex<ProgramCounters>` are gated behind `--timings`, removing that lock from
  the hot `SymbolTable::get` and clone paths in normal runs while keeping the
  counts exact when `--timings` collects them. (3) `SymbolTable` is now
  copy-on-write: the inner map is held as `Arc<HashMap<..>>`, `clone` is an `Arc`
  bump, and the mutating methods go through a `symbols_mut` helper that
  `Arc::make_mut`s and records the entry/handle copies only when a shared table
  is actually mutated. This makes the multi-pass module-binding fixpoint's ~9143
  table clones cheap without touching the fixpoint logic, taking
  `symbol_table_entry_handle_copy_count` from `86782` to `27698` and
  `symbol_info_handle_copy_count` from `92072` to `32988`. (4)
  `resolve_relative_module` is memoized per run via a thread-local
  `(importer, specifier) -> Option<ModuleResolution>` cache (cleared at check
  start, since resolved indices are run-specific), so fixpoint passes after the
  first reuse the resolved index instead of rebuilding and canonicalizing
  candidate paths. The measured auth-kit medians improve from v1.2.4's `0.80s` /
  `0.78s` to roughly `0.20s` / `0.19s` for `jobs=1` / `jobs=4` (stable floor near
  `0.18s`). Profiling (`/usr/bin/sample`) showed the dominant pre-fix cost was
  uncached `realpath`, not type-payload cloning; the remaining hot cost is the
  multi-pass binding/resolution recompute itself, deferred to a future
  correctness-sensitive fixpoint-reduction pass. No hot allocator mutex was
  introduced and the prior handle-backed migrations are preserved.

- v1.2.4 is a performance recovery / stabilization pass after v1.2.3, not a
  new TypeScript semantic phase. No new TypeScript surface was added. v1.2.3
  `SymbolInfo` shared-handle storage is preserved while function-local variable
  checking borrows visible symbols instead of cloning whole tables, function
  signature setup lazily clones parameter scopes only when parameter
  initializers need them, and `ScopeStack` restores per-frame visible-symbol
  shadows instead of eagerly rebuilding the flat visible table on every pop. On
  the latest auth-kit measurement, exact diagnostics remain 0 and raw oracle
  match stays yes. Module-export reductions from v1.2.2 are preserved with
  `function_type_copy_from_module_export_count=0` and
  `union_type_copy_from_module_export_count=0`. The measured auth-kit medians
  are `0.80s` at `jobs=1` and `0.78s` at `jobs=4`, improved from v1.2.3's
  `0.85s`/`0.88s` but not fully back to v1.2.2's `0.67s`/`0.65s`. Current
  handle counters are `function_type_handle_copy_count=2349`,
  `union_type_handle_copy_count=1181`, `object_type_payload_deep_clone_count=0`,
  `function_type_payload_deep_clone_count=0`, and
  `union_type_payload_deep_clone_count=0`. `scope_or_context` attribution
  remains near zero at `1` for function handles and `11` for union handles.
  Remaining symbol/scope pressure is reported honestly:
  `symbol_info_handle_copy_count=92072`, `symbol_table_clone_count=9143`,
  `symbol_table_entry_handle_copy_count=86782`,
  `scope_stack_visible_rebuild_count=0`, and
  `scope_stack_visible_symbol_handle_copy_count=513`. Remaining
  `symbol_info_payload_deep_clone_count=6` is rare replacement/construction
  work. TypeDeclarationTable/ObjectType/FunctionType/UnionType handle-backed
  migrations remain preserved, and no hot allocator mutex was introduced.

- v1.2.1 is an attribution-first stabilization pass, not a new semantic phase.
  No new TypeScript behavior was added. On the latest auth-kit measurement,
  exact diagnostics remain 0 and raw oracle match stays yes, but the total
  handle-copy counts did not drop yet: `function_type_handle_copy_count`
  remains `946298` and `union_type_handle_copy_count` remains `10047`. The
  attribution surface is now materially better: function copies are mostly from
  `module_export=378735`, `function_body_setup=211137`, and
  `scope_or_context=194561`, with `function_type_copy_unattributed_count=156626`
  still remaining; union copies are mostly from `module_export=3155`,
  `scope_or_context=2248`, and `function_body_setup=1970`, with
  `union_type_copy_unattributed_count=1672` remaining. Both payload deep clone
  counts stay at zero. The next phase should target one of the attributed hot
  sources instead of broadening the clone surface again.

- v1.2 is a performance-first stabilization pass, not a new semantic-expansion
  phase. No new TypeScript surface was added. On the latest auth-kit
  measurement, exact diagnostics remain 0 and raw oracle match stays yes, but
  the handle-copy reduction is still modest:
  `function_type_handle_copy_count=946298` (down from `946413`) and
  `union_type_handle_copy_count=10047` (down from `10520`). `jobs=1` is
  `0.98s` and `jobs=4` is `1.00s`, so wall-clock time has not improved yet.
  The timing dump now shows `type_declaration_collection=649.339ms`,
  `module_binding=364.431ms`, `per_file_statement_checking=39.620ms`,
  `flow_narrowing=44.889ms`, `function_declaration_checking=37.614ms`,
  `object_literal_checking=8.720ms`, `call_expression_checking=1.268ms`, and
  `assignability_checking=0.461ms`. The current attribution surface only shows
  `55` function copies and `100` union copies from expression identifier
  lookups; the remaining copies still sit elsewhere in the function-body and
  call-checking paths.

- v1.1 supports narrow generic indexed access after concrete substitution, including `T["key"]`, `T[K]`, and `T[keyof T]` when the receiver/key have been substituted to concrete types. Fully unresolved generic indexed access and constraint enforcement remain unsupported. `generic-indexed-access-basic` now matches TypeScript on that boundary. Auth-kit stays exact at 0 diagnostics, raw oracle match stays yes, compatReport diagnosticsTotal stays 0, `suppressedRustOnlyDiagnosticsTotal` remains 20 in the tsc-profile report, and the measured auth-kit counters still show the same handle-backed composite state with `ObjectType`, `FunctionType`, and `UnionType` backed by shared handles and `TypeDeclarationTable` preserved from v0.96.

- v1.0.1 stabilizes narrow generic call-site inference for simple direct calls, repeated-parameter calls, and array-element calls. Explicit type arguments still instantiate the generic call and tuple-valued implicit generic returns remain suppressed on the inferred path. Auth-kit stays exact at 0 diagnostics, raw oracle match stays yes, compatReport diagnosticsTotal stays 0, and `suppressedRustOnlyDiagnosticsTotal` remains 20 in the tsc-profile report. On the measured auth-kit project, `ts-rust` is `0.96s` at `jobs=1` and `0.92s` at `jobs=4`; the timing buckets are `type_declaration_collection=630.622ms`, `module_binding=348.007ms`, `per_file_statement_checking=39.147ms`, `flow_narrowing=44.484ms`, `function_declaration_checking=37.413ms`, `object_literal_checking=8.416ms`, `call_expression_checking=2.198ms`, and `assignability_checking=0.451ms`. The new generic inference counters show `attempts=12`, `successes=0`, `failures=2`, `explicit skips=10`, `unresolved skips=1`, `tuple suppressions=0`, and `candidates=0` on auth-kit. `ObjectType`, `FunctionType`, and `UnionType` remain handle-backed, `TypeDeclarationTable` stays arena-backed from v0.96, and no hot allocator mutex was introduced.

- v0.99 completes the composite-type handle sequence by moving `UnionType`
  payloads behind shared handles while preserving the earlier `ObjectType`
  and `FunctionType` migrations and the v0.96 `TypeDeclarationTable`
  arena-backed payloads. Auth-kit stays at 0 diagnostics, raw oracle match
  stays yes, compatReport diagnosticsTotal stays 0, and
  `suppressedRustOnlyDiagnosticsTotal` remains 20 in the tsc-profile report.
  `contextual-callback-object-properties-basic`, `mapped-types-basic`,
  `type-operators-basic`, and `indexed-access-basic` still match TypeScript.
  On the measured auth-kit project, the benchmark medians are `0.95s` at
  `jobs=1` and `0.92s` at `jobs=4` for `ts-rust`, while `tsc` is `1.12s` and
  `1.11s`, `tsgo` is `0.43s` and `0.43s`, and `tsgo-singleThreaded` is
  `0.55s` and `0.53s`, so this slice is structural cleanup rather than a
  dramatic wall-clock win. The release timing buckets are
  `type_declaration_collection=632.120ms`, `module_binding=350.221ms`,
  `per_file_statement_checking=38.130ms`, `flow_narrowing=43.167ms`,
  `function_declaration_checking=36.444ms`, `object_literal_checking=8.423ms`,
  `call_expression_checking=1.208ms`, and `assignability_checking=0.462ms`.
  The counters now show `checker_arena_alloc_count=25491`,
  `arena_object_type_payload_alloc_count=1993`,
  `object_type_payload_deep_clone_count=0`, `object_type_clone_count=280`,
  `object_type_id_copy_count=280`, `function_type_payload_alloc_count=2461`,
  `function_type_payload_deep_clone_count=0`,
  `function_type_handle_copy_count=946413`,
  `function_type_clone_count=946413`, `union_type_payload_alloc_count=1851`,
  `union_type_payload_deep_clone_count=0`,
  `union_type_handle_copy_count=10516`, `union_type_clone_count=100`, and
  `type_clone_count=771`. `TypeDeclarationTable` stays arena-backed from v0.96,
  `ObjectType` stays handle-backed from v0.97, and `FunctionType` stays
  handle-backed from v0.98; no hot allocator mutex was introduced.

- v0.97.1 stabilizes the v0.97 object-slice landing instead of starting a new
  arena/type-IR phase. `contextual-callback-object-properties-basic` and
  `mapped-types-basic` now match TypeScript again, auth-kit stays exact at 0
  diagnostics, raw oracle match stays yes, compatReport diagnosticsTotal stays
  0, and `suppressedRustOnlyDiagnosticsTotal` remains 20 in the tsc-profile
  report. `ObjectType` payloads still live behind shared handles, `FunctionType`
  and `UnionType` remain value-owned, and no UnionType/FunctionType migration
  has started.

- v0.97 keeps auth-kit exact at 0 diagnostics and moves `ObjectType` payloads
  onto shared handles instead of repeating deep clones of the property map.
  `ObjectType` construction now goes through a checker-side allocation helper,
  so `Type::Object` clone paths copy handles while object payload deep clones
  drop to zero. On the measured auth-kit project, the benchmark medians are
  `0.94s` at `jobs=1` and `0.90s` at `jobs=4`, with timing buckets of
  `type_declaration_collection=1167.696ms`, `module_binding=542.743ms`,
  `import_binding_resolution=407.085ms`, `per_file_statement_checking=305.801ms`,
  `function_declaration_checking=294.843ms`, and `flow_narrowing=372.755ms`.
  The counters now show `checker_arena_alloc_count=23067`,
  `arena_declaration_key_alloc_count=10538`,
  `arena_type_declaration_payload_alloc_count=10538`,
  `arena_object_type_payload_alloc_count=1991`,
  `type_declaration_payload_deep_clone_count=15319`,
  `object_type_payload_deep_clone_count=0`, `type_clone_count=763`,
  `object_type_clone_count=275`, `object_type_id_copy_count=275`,
  `union_type_clone_count=98`, `symbol_name_clone_count=0`,
  `string_key_clone_count=0`, `flow_local_name_clone_count=0`,
  `string_path_lookup_count=30470`, and
  `canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw
  oracle match stayed yes, compatReport diagnosticsTotal stayed 0, and
  `suppressedRustOnlyDiagnosticsTotal` is 20 in the tsc-profile report.
  `FunctionType` and `UnionType` remain value-owned, so this is still a
  handle-backed slice rather than a full type-arena migration.

- v0.96 is a confirmed real payload migration, not a key-only landing. `TypeDeclarationTable` now stores `TypeDeclarationInfo` payloads as arena-owned handles behind `TypeDeclarationId` entries while declaration names remain in arena-backed `ArenaStr` keys. The arena is program-local and cloned read-only into worker contexts, so lowering allocates once and table clone paths copy IDs/handles instead of payload bodies. On the measured auth-kit project, the benchmark medians moved from `3.04s` to `2.36s` at `jobs=1` and from `2.54s` to `2.10s` at `jobs=4`. The timing dump now shows `type_declaration_collection` at `1140.712ms`, `module_binding` at `457.063ms`, `import_binding_resolution` at `290.375ms`, `per_file_statement_checking` at `656.381ms`, `function_declaration_checking` at `617.091ms`, and `flow_narrowing` at `598.324ms`. The counters now show `checker_arena_alloc_count=21076`, `arena_declaration_key_alloc_count=10538`, `arena_type_declaration_payload_alloc_count=10538`, `type_declaration_table_clone_count=4`, `type_declaration_id_copy_count=1579`, `type_declaration_payload_deep_clone_count=15319`, `type_declaration_entries_merged_total=863`, `type_clone_count=763`, `object_type_clone_count=275`, `union_type_clone_count=98`, `symbol_name_clone_count=0`, `string_key_clone_count=0`, `flow_local_name_clone_count=0`, `string_path_lookup_count=30470`, and `canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. Reviewers should inspect `crates/typescript-rust-checker/Cargo.toml`, `crates/typescript-rust-checker/src/arena.rs`, `crates/typescript-rust-checker/src/lib.rs`, `crates/typescript-rust-checker/src/symbols/type_declarations.rs`, `crates/typescript-rust-checker/src/program.rs`, `crates/typescript-rust-checker/ARENA_ID_PLAN.md`, `REAL_PROJECT_COMPAT.md`, `crates/typescript-rust-checker/REAL_PROJECT_COMPAT.md`, and `.bench/auth-kit-measurement.md` for the evidence surface.

- v0.95: auth-kit stays at 0 diagnostics while the checker lands its first live `oxc_allocator` slice. `TypeDeclarationTable` now interns declaration names into arena-backed `ArenaStr` keys through a program-local `CheckerArena`, while the declaration payloads remain value-owned. On the measured auth-kit project, the benchmark medians moved from `3.1008863750000017s` to `3.04s` at `jobs=1` and from `2.568661s` to `2.54s` at `jobs=4`. The timing dump now shows `type_declaration_collection` at `2222.851ms`, `module_binding` at `1056.509ms`, `import_binding_resolution` at `924.218ms`, `per_file_statement_checking` at `666.348ms`, `function_declaration_checking` at `626.823ms`, `flow_narrowing` at `617.100ms`, and `declaration_table_merging_cloning` at `2.504ms`. The counters now show `checker_arena_alloc_count=10538`, `type_arena_alloc_count=10538`, `type_declaration_table_clone_count=4`, `type_declaration_entries_cloned_total=1579`, `type_declaration_entries_merged_total=863`, `type_clone_count=763`, `object_type_clone_count=275`, `union_type_clone_count=98`, `symbol_name_clone_count=0`, `string_key_clone_count=0`, `flow_local_name_clone_count=0`, `string_path_lookup_count=30470`, and `canonical_file_id_lookup_count=14574`. Exact diagnostics stayed at 0, raw oracle match stayed yes, and compatReport diagnosticsTotal stayed 0. The arena-backed slice covers declaration-key interning only; payload cloning still happens on `TypeDeclarationInfo` and the remaining module/function/flow paths.

- v0.82: project/file visibility hardening, including recursive directory includes and `.tsx` visibility.
- v0.83: parser-safe binding-pattern parameter support for `TS7031` on object binding elements in function and arrow parameters.
- v0.84.5: deterministic parallel project-checking foundation. This only changes how per-file work is scheduled in project mode; it does not add new semantic, resolver, lib, or declaration behavior.
- v0.84.8: real-source syntax/scope reconciliation fixtures for optional typed parameters, async locals, destructuring locals, nested object shorthand, early returns, type import visibility, and a narrow `TextEncoder` builtin.
- v0.85: generated default-lib foundation from the local TypeScript package, including ambient core and DOM subset loading plus `noLib` disabling.
- v0.94: generated default-lib runtime parsing/lowering is removed for the generated core/DOM subset, and package declaration `.d.ts` files under `node_modules/**/dist/` stay in the dependency-declaration path instead of being misclassified as generated libs. On the measured auth-kit project, `generated_default_lib_files=2`, `parsed_generated_default_lib_files=0`, `generated_default_lib_parse_time=0.000ms`, `generated_default_lib_lower_time=0.036ms`, `dependency_declaration_parse_time=4.907ms`, and `dependency_declaration_lower_time=0.910ms`. The benchmark medians moved from `3.262684000000001s` to `3.1008863750000017s` at `jobs=1` and from `2.767987000000001s` to `2.568661s` at `jobs=4`. Exact diagnostics stay at 0, raw oracle match stays yes, and compatReport diagnosticsTotal stays 0. The current runtime work is still import resolution, declaration collection, and flow/function checking; the live `TypeArena` slice has not landed yet, so its alloc counters remain zero.
- v0.93: auth-kit stays exact at 0 diagnostics while Arc-backed interning removes the remaining symbol/string/flow-local clone hot spots. On the measured auth-kit project, benchmark medians moved from `2.720818041999999s` to `2.65s` at `jobs=1` and from `2.2235199169999977s` to `2.15s` at `jobs=4`. The timing dump now shows `type_declaration_collection` at `1.152298s`, `module_binding` at `483.614ms`, `import_binding_resolution` at `317.398ms`, `per_file_statement_checking` at `653.938ms`, `function_declaration_checking` at `614.590ms`, and `flow_narrowing` at `597.704ms`. The symbol/string clone counters dropped to zero, while `string_path_lookup_count=30503` and `canonical_file_id_lookup_count=14574` stayed flat, so file/module identity lookup is the next measurable bottleneck. Exact diagnostics remain at 0, raw oracle match remains yes, and compatReport diagnosticsTotal remains 0.
- v0.86: auth-kit stays at 0 diagnostics while module binding avoids repeated loaded-file scans via canonical identity lookup, and the timing buckets now expose the dominant declaration-collection and export-resolution loops. On the measured auth-kit project, `module_binding` fell from 22.731s to 2.049s and `type_declaration_collection` from 11.041s to 3.743s, with benchmark medians improving from 29.34s to 7.42s at `jobs=1` and from 28.47s to 6.20s at `jobs=4`.
- v0.87: auth-kit stays at 0 diagnostics while the final module-analysis pass reuses the preliminary module type declarations instead of re-lowering them. On the measured auth-kit project, `type_declaration_collection` moved to 3.307s and `module_binding` to 1.835s, with benchmark medians improving further to 6.30s at `jobs=1` and 5.67s at `jobs=4`.
- v0.88: auth-kit stays at 0 diagnostics while the declaration path is now instrumented with hard clone/merge and lookup counters. On the measured auth-kit project, the benchmark medians moved to 6.22s at `jobs=1` and 5.51s at `jobs=4`, with `type_declaration_collection` at 4.839s, `module_binding` at 1.758s, 650 module-analysis calls, 3,909 table clones, and 2,927 merges. The target is still not met, and declaration collection plus table materialization remains the next structural bottleneck.
- v0.89: auth-kit stays at 0 diagnostics while declaration lookup moves to a layered scope instead of repeatedly materializing merged ambient/dependency tables. On the measured auth-kit project, the benchmark medians improved to 2.84s at `jobs=1` and 2.34s at `jobs=4`, with `type_declaration_collection` at 1.199s, `module_binding` at 496ms, `declaration_table_merging_cloning` at 2.535ms, 4 table clones, 327 merges, 1,629 cloned entries, 863 merged entries, 0 generated-default-lib table clones, 0 dependency-declaration table clones, and `declaration_lookup_layer_count_avg` at 1.14. The current bottleneck has shifted away from ambient table materialization toward per-file statement checking and the remaining import/validation work.
- v0.90: auth-kit stays at 0 diagnostics while the statement-checking hot path stops rebuilding merged scope tables on every read. On the measured auth-kit project, the benchmark medians improved to 2.76s at `jobs=1` and 2.26s at `jobs=4`, with `type_declaration_collection` at 1.204s, `module_binding` at 499.466ms, `import_binding_resolution` at 320.765ms, `per_file_statement_checking` at 678.533ms, and `declaration_lookup_layer_count_avg` still at 1.14. The nested statement buckets show `function_declaration_checking` at 637.294ms, `flow_narrowing` at 628.468ms, `variable_declaration_checking` at 242.589ms, `return_statement_checking` at 156.014ms, `object_literal_checking` at 150.116ms, `assignability_checking` at 10.586ms, and `call_expression_checking` at 2.832ms. The counters now show 1,963 expression checks, 1,840 expression inferences, 380 property lookups, 136 call resolutions, 158 object-literal property checks, 333 function-body checks, and 772 type clones. The oracle compare remains exact at 0 diagnostics, so the remaining bottleneck is the function-body and flow work, not declaration materialization.
- v0.91: auth-kit stays at 0 diagnostics while flow checking gets a cheap relevance prepass, skips expression-flow traversal when nothing is tracked, and exposes dedicated flow counters. On the measured auth-kit project, the benchmark medians improved to 2.72s at `jobs=1` and 2.22s at `jobs=4`, with `type_declaration_collection` at 1.195s, `module_binding` at 508.418ms, `import_binding_resolution` at 319.178ms, `per_file_statement_checking` at 665.498ms, and `declaration_lookup_layer_count_avg` still at 1.14. The nested statement buckets show `function_declaration_checking` at 625.309ms, `flow_narrowing` at 612.869ms, `variable_declaration_checking` at 237.916ms, `return_statement_checking` at 149.474ms, `object_literal_checking` at 143.692ms, `assignability_checking` at 10.316ms, and `call_expression_checking` at 2.851ms. The flow counters now show `flow_function_count=333`, `flow_function_skipped_count=41`, `flow_statement_count=678`, `flow_expression_visit_count=1806`, `flow_identifier_read_count=759`, `flow_scope_push_count=78`, `flow_scope_pop_count=78`, `flow_future_declaration_collection_count=292`, `flow_future_declaration_entries_total=235`, `flow_state_clone_count=616`, `flow_scope_locals_clone_count=2347`, `flow_branch_merge_count=123`, `flow_branch_merge_scope_count=154`, `flow_read_lookup_count=759`, `flow_read_lookup_scope_steps_total=850`, `flow_return_analysis_walk_count=505`, and `flow_truthiness_check_count=122`. The oracle compare remains exact at 0 diagnostics, so the remaining bottleneck is still the function-body/flow work, with import resolution still measurable.
- v0.92: auth-kit stays at 0 diagnostics while branch-state cloning is replaced by a branch snapshot/delta merge path. On the measured auth-kit project, the benchmark medians held at 2.720818041999999s at `jobs=1` and 2.2235199169999977s at `jobs=4`, so the wall-clock effect is neutral for now even though the flow clone counters collapsed to zero. The timing dump now shows `type_declaration_collection` at 1.205879s, `module_binding` at 514.167ms, `import_binding_resolution` at 317.343ms, `per_file_statement_checking` at 693.565ms, `function_declaration_checking` at 648.517ms, and `flow_narrowing` at 626.458ms. The flow counters now show `flow_function_count=333`, `flow_function_skipped_count=41`, `flow_statement_count=678`, `flow_expression_visit_count=1806`, `flow_identifier_read_count=759`, `flow_scope_push_count=78`, `flow_scope_pop_count=78`, `flow_future_declaration_collection_count=292`, `flow_future_declaration_entries_total=235`, `flow_state_clone_count=0`, `flow_scope_locals_clone_count=0`, `flow_state_full_clone_avoided_count=370`, `flow_branch_merge_count=123`, `flow_branch_merge_scope_count=154`, `flow_branch_merge_local_iteration_count=22`, `flow_branch_merge_fast_path_count=120`, `flow_branch_empty_delta_count=135`, `flow_branch_changed_local_count=235`, `flow_read_lookup_count=759`, `flow_read_lookup_scope_steps_total=850`, `flow_return_analysis_walk_count=547`, and `flow_truthiness_check_count=122`. The hot-path clone counters now also show `type_clone_count=772`, `object_type_clone_count=278`, `union_type_clone_count=98`, `symbol_name_clone_count=1329049`, `string_key_clone_count=143234`, `flow_local_name_clone_count=711`, `type_name_lookup_string_count=12502`, `string_path_lookup_count=30503`, and `canonical_file_id_lookup_count=14574`. Exact diagnostics remain at 0, raw oracle match remains yes, and compatReport diagnosticsTotal remains 0. The next bottleneck is still function-body/flow work plus import resolution, with return-flow walks the clearest follow-on target. The v0.93 preflight points to `FileId`/`ModuleId`/`SymbolId` interning before any broader `TypeArena` spike.

auth-kit currently matches TypeScript with 0 diagnostics under the measured
command set.

## Still out of scope

- Full JSX semantics.
- Full lib.d.ts parity beyond the generated subset.
- Node and `@types` discovery.
- Full package resolution and package runtime exports resolution.
- `baseUrl`, project references, and broader module-resolution heuristics.
- Full callback contextual typing or generic callback inference.
- Tuple-valued implicit generic returns.
- Full destructuring semantics, including array and rest binding modeling beyond parser safety.

The compatibility target for this phase remains `tsc` profile oracle comparisons on loaded real projects, not native-profile ergonomics. `tsc` remains the default diagnostic profile; `native` is opt-in.
