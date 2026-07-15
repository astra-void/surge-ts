# Memory Regions

This note records the checker's memory-lifetime inventory, the region model the
pipeline actually implements, and the reset/drop boundaries that enforce it.
It is the companion to [ARENA_ID_PLAN.md](ARENA_ID_PLAN.md) (arena/handle
representation) and to the RSS stage instrumentation in `metrics/`.

## Region hierarchy

```text
Compilation (one check_program_with_stats_and_jobs run)
├── ParserWorkerRegion × parse worker      one oxc Allocator per thread, dropped after parse
├── ProgramRegion                          lives to end of run
│   ├── CheckerArena(s)                    bump storage for declaration keys/payloads; frozen
│   │                                      before the parallel fan-out, never reset mid-run
│   ├── ambient tables                     ambient_global_{symbols,type_declarations}, ambient_modules
│   ├── shared_state                       global/script declaration tables, global symbols,
│   │                                      final module analyses/import bindings/resolution scopes
│   └── module_scope_by_file / module_local_values_by_file
├── PhaseRegion (binding fixpoint)         preliminary module analyses, preliminary import
│   │                                      bindings/scopes, per-round export tables
│   └── dropped at `preliminary_release` once the final round supersedes them
├── WorkerRegion × check worker            one CheckerContext clone, reused across files
│   ├── FileCheckRegion                    per-file symbol/declaration environment, per-file
│   │                                      resolved_named_types cache, utility diagnostic keys
│   ├── FunctionCheckRegion                FunctionFlowState, scope stacks — function-local by
│   │                                      construction (created per body, dropped on return)
│   └── recursion scratch                  resolving stacks, peel stack, assignability
│                                          depth/visited — unwind-scoped thread state
└── CacheRegion                            program_resolved_generic_types, program_instantiations,
                                           per-run thread-local path/module caches
```

Rules the code enforces:

- Short-lived regions may reference longer-lived regions (worker state reads
  frozen program tables). Program-lifetime structures never store references
  into per-file or per-function state; results that must survive a boundary are
  promoted (interned into the program caches, or moved out as owned
  `Diagnostic`s in `FileCheckResult`).
- Cache lifetime is separate from correctness-critical program data: every
  entry in the CacheRegion is a memo of a recomputable resolution. The caches
  are torn down at end of run (`clear_program_type_caches`) to break the
  snapshot Arc cycle; the per-run thread-local caches are cleared at run start.
- No region reset bypasses destructors: only the `CheckerArena` bump storage is
  drop-free, and it stores only `Drop`-free payload shapes behind write-once
  handles (see arena.rs safety notes).

## Lifetime inventory

Classification legend: `program` (whole run), `phase` (one pipeline phase),
`file`, `function`, `worker-scratch` (reusable per worker), `bounded-cache`,
`owned-output`.

| Structure | Owner / storage | Class | Actual lifetime notes |
| --- | --- | --- | --- |
| `SourceFileInput.source_text` | checker `files` vec | phase | dropped when `parse_program_files` returns; the CLI keeps its own copy in `ProjectCheckResult.sources` for code frames (see "Remaining retention") |
| `ParsedProgramFile.statements` | `parsed_files` | program | needed by binding and the check phase; declaration-file ASTs are freed before checking under `skipLibCheck` (`declaration_ast_release` stage) |
| Declaration payloads (`InterfaceInfo`/`TypeAliasInfo` bodies) | `CheckerArena` + `Arc` bodies | program | write-once; shared by handle |
| `ambient_global_*`, `ambient_modules` | `CheckerContext`, `Arc` | program | built during ambient collection, read-only afterwards |
| Preliminary module analyses / import bindings / scopes | orchestrator locals | phase | superseded by the final analysis round; dropped at `preliminary_release` (previously lived to end of run) |
| Final `module_analyses` / `module_import_bindings` / `module_resolution_scopes` | `shared_state` | program | any worker may check any file, so the full set stays live through the check phase |
| Per-round `module_export_tables` | orchestrator local | phase | last read by the JSX intrinsic locator; dropped at `preliminary_release` |
| `local_type_declarations_by_module` | orchestrator local | phase | last read by the final scope build; dropped at `preliminary_release` |
| Worker `CheckerContext` clone | check worker | worker-scratch | one clone per worker, mutated in place per file |
| `ctx.symbols`, `ctx.type_declarations`, `ctx.type_declaration_scope`, `module_value_fallback` | worker context | file | replaced at each file boundary by `check_program_file` |
| `ctx.resolved_named_types` | worker context, `Arc<Mutex<HashMap>>` | file | per-file memo; the map is swapped (not cleared in place) because lazy-resolution snapshots hold the old `Arc` |
| `ctx.utility_diagnostic_keys` | worker context | file | keys embed the file name; cleared at file begin so a reused worker context does not accumulate every file's keys |
| `ctx.diagnostics` + dedup keys | worker context | file → owned-output | `mem::take`n into `FileCheckResult` per file |
| `FunctionFlowState` (flow scopes, branch captures, alias guards) | function-local | function | created per function body, dropped on return |
| `type_parameter_scopes` / constraint scopes / namespace prefix stack / structural frames | worker context | function | balanced push/pop inside a file |
| `resolving` stacks, `LAZY_PEEL_STACK`, assignability depth/visited | function-local / thread-local | function | unwind-scoped; bounded by explicit depth caps |
| `program_resolved_generic_types` | `Arc<Mutex<HashMap>>` | bounded-cache | per-declaration bucket cap (`GENERIC_INSTANTIATION_BUCKET_CAP`); cleared at end of run |
| `program_instantiations` | `Arc<Mutex<HashMap>>` | bounded-cache | same bucket cap; holds the interned reference expansions |
| `CANONICALIZE_CACHE`, `RELATIVE_MODULE_CACHE`, `STAR_EXPORT_UNRESOLVED_CACHE`, `NAMESPACE_ALIAS_TABLE_CACHE` | thread-local | bounded-cache (per run) | cleared at run start on the main thread; worker threads are fresh per run |
| `lazy_resolution_snapshot` | worker context, `Arc<CheckerContext>` | program (per worker) | captured once per context on first deferred library reference; pinned by every `LazyInstantiation` created from it, so it cannot be reset per file |
| Diagnostics | `FileCheckResult` → merged vec | owned-output | ordinary owned values with destructors |

## Cache classification

| Cache | Class | Policy |
| --- | --- | --- |
| `resolved_named_types` | file-local cache | fresh map per checked file (stale cross-file entries would be incorrect: resolution depends on the consumer file's environment) |
| `program_resolved_generic_types` | program-wide bounded cache | bucket cap per declaration; over-cap entries recompute; structural-equality lookup makes collisions harmless |
| `program_instantiations` | program-wide bounded cache | same cap; first-wins; degraded (`had_error`) expansions are never interned |
| relative-module / canonicalize / star-export / namespace-alias thread-locals | program-wide mandatory memoization (per run) | keyed by the fixed file set; cleared at run start |
| `EQ_PROBE_VISITS` (`SURGE_EQ_STATS` only) | diagnostic probe | unbounded across runs by design; marked for removal with the probe |

## Lifetime mismatches found (and their fixes)

1. Preliminary module analyses, preliminary import bindings, preliminary
   resolution scopes, `local_type_declarations_by_module`, and the final
   `module_export_tables` all lived to end of run while the parallel check
   phase — the peak-RSS phase — ran. Fixed: dropped at the
   `preliminary_release` boundary after their last reads (eq probe, binding
   merge, final scope build, JSX locator).
2. `ctx.utility_diagnostic_keys` accumulated every checked file's keys on a
   reused parallel worker context (serial checking clones a fresh context per
   file, so only parallel runs grew). Fixed: cleared in `begin_file_check`.
3. `signature_ctx` (a full per-file context clone for signature collection)
   duplicated per-file scaffolding; retained, but its diagnostics/keys are
   cleared on construction. Cost is Arc bumps; measured as noise.
4. Statement checking deep-clones each top-level `ParsedStatement`
   (`statements.rs`), because the check paths consume owned statements. This is
   churn, not retention; converting the check paths to borrows is a large
   refactor left as a follow-up (see "Remaining retention").
5. The CLI retains two copies of every source text during checking (`inputs`
   for the checker + `sources` for code-frame rendering); the checker copy is
   freed after parsing, the render copy lives to process exit. Documented, not
   yet changed: `SourceFileInput.source_text` is a public `String` field and
   render-time re-reads would change observable behavior on files modified
   mid-run.

## Remaining retention sources (largest first)

- `shared_state` module analyses/bindings/scopes for the whole check phase —
  intrinsic to "any worker checks any file"; would need file-affinity
  scheduling to shrink.
- The interned type graph (`program_instantiations` + expansions) — bounded per
  bucket, freed at end of run.
- Root-source ASTs through the check phase (declaration ASTs are already
  released under `skipLibCheck`).
- The CLI `sources` copy of every input text (render lifetime).
- Per-statement clone churn in `check_program_file_statements`.
