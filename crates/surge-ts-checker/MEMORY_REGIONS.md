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
- No region reset bypasses destructors: `CheckerArena` bump storage registers
  every `Drop`-requiring payload in a `pending_drops` list at allocation time
  and runs each payload's typed `drop_in_place` exactly once when the last
  arena handle drops (see arena.rs safety notes). Trivially droppable payloads
  carry no destructor metadata. Before this registration existed, payloads
  allocated through `MaybeUninit` never ran `Drop`, leaking every declaration
  payload's `String`s and `Arc` refcounts to process exit (~400 MB of the tRPC
  finish footprint).

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

## Memory-lifetime program (2026-07-17)

The retained-memory series (`f518a81..8f0c3a9`, tag `trpc-memory-1.8gb`) cut
the tRPC peak physical footprint from ~3.8 GB to ~1.8 GB and the finish
footprint from ~1.97 GB to ~0.56 GB with byte-identical diagnostics. Full
evidence and rejected designs: [docs/MEMORY-OPTIMIZATION-REPORT.md](../../docs/MEMORY-OPTIMIZATION-REPORT.md).
The load-bearing mechanisms, which later work must preserve:

- **Weak canonical-store retention** (`surge-ts-types/src/store.rs`): bucket
  entries for functions, parameter lists, unions, and property maps hold
  `Weak` payload references; a payload lives exactly as long as some consumer
  holds its `Arc`. IDs are monotonic and never reused, so ID-equality fast
  paths cannot ABA; expired entries are swept on the next bucket scan and an
  equivalent payload re-interns under a fresh ID. Do not convert a store back
  to strong retention without measured justification.
- **Arena Drop registration** (`arena.rs` `pending_drops`): see the rule above.
  Any new arena payload shape that owns heap data must be registered exactly
  once.
- **Compact declaration environments**: an environment captures an
  `Arc<TypeDeclarationTable>` snapshot deduplicated by the table's
  `(instance_id, version)` mutation stamp — one shared snapshot per mutation
  burst, zero full-table clones per environment — plus span-free symbol
  tables (`clone_for_environment_capture`) and an empty working value table.
  `typeof` and value-dependent lookup resolve through ambient →
  `module_value_fallback` → `module_local_values_by_file`. Environments must
  not capture span maps, value tables, diagnostics, flow state, or checker
  context: the pre-fix representation retained ~1.03 GB, including 5.9 M
  COW-defeated span-map entries.
- **Qualified-import payload sharing with owning-arena retention**:
  `TypeDeclarationTable::insert_shared_from` adopts the exporter's payload
  pointer instead of deep-copying per importer, and retains the payload's
  owning arena in `foreign_payload_arenas` so a shared handle can never
  outlive its arena. `get_handle` hands back the true owning arena.
- **True-death lifecycle releases**: declaration-file AST bodies are filtered
  to import/export statements after final module analysis; superseded
  binding/scope generations drop before each rebuild (never three generations
  live); the serial check loop frees each file's AST, `module_analyses[i]`,
  and `module_import_bindings[i]` progressively; `shared_state`/`parsed_files`
  drop before the finish measurement; run-scoped TLS caches clear at teardown;
  `malloc_zone_pressure_relief` runs at generation boundaries and every 256
  files (macOS-only, supplementary — never a substitute for ownership fixes).

Instrumentation: `SURGE_RETENTION_CENSUS=1` walks the retained heap at each
lifecycle boundary with per-owner-group attribution (opt-in,
diagnostics-neutral); `SURGE_PAUSE_AT_STAGE=<label>` self-SIGSTOPs for
`vmmap`/`malloc_history` attachment; `SURGE_RSS=1` prints the per-stage
`fp`/`fp_peak` physical-footprint columns.

Measured-and-rejected lifetime shortcuts (do not retry without a semantic
redesign): sharing `export *` re-export payloads across tables, and pruning
`program_instantiations` entries by strong-count — both shift which first-wins
expansion later consumers observe and drift zod diagnostics. Expansion-cache
lifetime is semantically load-bearing; only true-death reclamation (an entry
dying because no consumer exists) is safe. Never share resolution results
keyed only on declaration identity when the result can depend on analysis
pass, lexical/module/type-parameter scope, import or augmentation generation,
recursion state, or resolution mode.

## Dependency declaration expansion policy

Installed-package `.d.ts`, `.d.mts`, and `.d.cts` files are classified as
`DependencyDeclaration` after module resolution has selected a physical path.
Their exported variable annotations, named aliases/interfaces, and
reference-only intersections remain declaration-backed references during
module analysis. A real semantic consumer may force structure for property or
call lookup, assignability, indexed/conditional/mapped evaluation, JSX, spread,
or destructuring. Discovery, import/re-export propagation, export-name
enumeration, display, cache keys, and export dedup do not force structure.

User declarations outside dependency roots, including path-mapped declarations
and project-reference outputs, stay `RootDeclaration` and use the conservative
user-authored checking policy. Generated declarations and physical default
libraries remain separately classified.

Set `SURGE_TRACE_DTS_EXPANSION=1` to emit physical-footprint high-water JSON and
an end-of-run summary of object creation, declaration expansions, reference
peels, generic instantiations, retained export-table nodes, and peel reasons.
The trace is opt-in because exact per-declaration aggregation is intentionally
more expensive than normal checking. `SURGE_EAGER_DEPENDENCY_ANNOTATIONS=1`,
`SURGE_EAGER_DEPENDENCY_ALIASES=1`, and
`SURGE_EAGER_REFERENCE_INTERSECTIONS=1` are profiling escape hatches for paired
comparisons, not production modes.

## Measured results (2026-07-15, macOS, system allocator, jobs=1)

Diagnostics were byte-identical before/after on all six reference projects
(auth-kit, ky, ofetch, trpc, unnamed, zod) at `--jobs 1`; `SURGE_TIMINGS`
counter diffs on trpc showed every shared work counter within 2%, i.e. the
region changes are work-neutral. Peak RSS (`/usr/bin/time -l`):

| project | before | after | note |
| --- | --- | --- | --- |
| zod | 823MB | 775MB | stable profile; pre-check steady state −43MB |
| trpc | 1.94GB | 1.83GB | high run-to-run variance, see below |
| auth-kit | 166MB | 155MB | |
| ofetch | 88MB | 84MB | |
| ky | 66MB | 65MB | |
| unnamed | 3.6GB | 3.6–5.1GB | inside its measured noise band |

Caveat: trpc and unnamed peak inside the module-analysis passes, whose
transient working set is resolution-order dependent (the same nondeterminism
behind the trpc 2019/2020 diagnostic wobble). Identical binaries measured
1.10–3.94GB (trpc) and 3.6–7.4GB (unnamed) across runs, so single-run peaks on
those projects are not meaningful; interleaved multi-run distributions and the
`SURGE_TIMINGS` counters are the evaluation tools.

End-of-run cache sizes on trpc (jobs=1): named-type cache 1.48M hits / 246k
inserts; generic-type cache 257k hits / 237k misses / 43k inserts;
instantiation interner 185k hits / 213k inserts; zero capped buckets at the
default 4096 cap.

## Evaluated and deliberately not implemented

- A `WorkerScratch` struct of reusable per-function containers: flow state
  (`FunctionFlowState`), resolution stacks, and flattening buffers are already
  function-local or unwind-scoped with explicit depth caps, so a pooled scratch
  would add plumbing without changing retention; the file boundary
  (`begin_file_check`) was the missing region reset and is implemented instead.
- Reusing the `resolved_named_types` map in place across files: entries depend
  on the consumer file's environment and lazy-resolution snapshots may still
  hold the previous file's `Arc`, so the per-file map swap is required for
  correctness; the per-file allocation is one `Arc<Mutex<HashMap>>`.
- Clearing `lazy_resolution_snapshot` per file: every `LazyInstantiation`
  created from the snapshot pins it anyway, so a per-file reset would create
  more snapshots, not fewer.
- Borrowed statement checking (removing the per-statement clone in
  `check_program_file_statements`): a large cross-cutting refactor of the check
  paths, deferred; it is churn, not retention.
- Typed-ID stores and string/path interning beyond the existing
  arena/`Arc<str>` sharing: not started in this pass per the staged plan — the
  next candidate is sharing `TypeDeclarationInfo` payloads as handles (see
  ARENA_ID_PLAN.md "Next Slice").
- CLI source-text double retention (`inputs` + `sources`): documented above;
  changing `SourceFileInput.source_text` breaks the public API and render-time
  re-reads change observable behavior; left as the next loader-side lever.

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
