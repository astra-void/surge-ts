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
│   │                                      (serial checking is a single worker of this kind:
│   │                                      one context clone reused for the whole pass)
│   ├── FileCheckRegion                    per-file symbol/declaration environment, per-file
│   │                                      resolved_named_types cache, utility diagnostic keys
│   ├── FunctionCheckRegion                FunctionFlowState, scope stacks — function-local by
│   │                                      construction (created per body, dropped on return)
│   └── recursion scratch                  resolving stacks, peel stack, assignability
│                                          depth/visited — unwind-scoped thread state
└── CacheRegion                            ProgramTypeStore (canonical payload interner),
                                           program_resolved_generic_types, program_instantiations,
                                           physical-interface caches, substitution store,
                                           declaration-environment store,
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
  are torn down at end of run (`clear_program_type_caches`, followed by
  `ProgramTypeStore::clear`) to break the snapshot Arc cycle; the per-run
  thread-local caches are cleared at run start.
- The canonical `ProgramTypeStore` (`surge-ts-types/src/store.rs`) is created
  per run and installed thread-locally on the main thread and every check
  worker. Its interned payloads are program-lifetime and write-once; its IDs
  embed a per-run owner tag and must never cross program owners.
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
| Worker `CheckerContext` clone | check worker | worker-scratch | one clone per worker, mutated in place per file; serial checking uses one reused clone for the whole pass (cloning per file was a measured ~3% of check time on tRPC) |
| `ctx.symbols`, `ctx.type_declarations`, `ctx.type_declaration_scope`, `module_value_fallback` | worker context | file | replaced at each file boundary by `check_program_file` |
| `ctx.resolved_named_types` | worker context, `Arc<Mutex<HashMap>>` | file | per-file memo; the map is swapped (not cleared in place) because lazy-resolution snapshots hold the old `Arc` |
| `ctx.utility_diagnostic_keys` | worker context | file | keys embed the file name; split into a shared pre-check baseline (captured on the first `begin_file_check`) plus a per-file overlay cleared at each file begin, with a retained-capacity bound so one pathological file cannot pin a huge table |
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
| `ProgramTypeStore` (functions/unions/parameter lists/property maps/overload merges) | program-wide canonical interner | sharded, uncapped; fingerprint-bucketed with exact structural-equality confirmation; cleared by `ProgramTypeStore::clear` at end of run |
| `physical_interface_*` caches + `SubstitutionStore` | program-wide cache | completed, clean physical-lib interface/member/overload expansions keyed by stable declaration + substitution + environment identity; cleared in `clear_program_type_caches` |
| `DeclarationEnvironmentStore` | program-wide interner | narrow captured declaration environments behind handles (instead of retained full contexts); cleared in `clear_program_type_caches` |
| relative-module / canonicalize / star-export / namespace-alias thread-locals | program-wide mandatory memoization (per run) | keyed by the fixed file set; cleared at run start; canonical paths shared as `Arc<str>` so hits are refcount bumps |
| `EQ_PROBE_VISITS` (`SURGE_EQ_STATS` only) | diagnostic probe | unbounded across runs by design; marked for removal with the probe |

## Lifetime mismatches found (and their fixes)

1. Preliminary module analyses, preliminary import bindings, preliminary
   resolution scopes, `local_type_declarations_by_module`, and the final
   `module_export_tables` all lived to end of run while the parallel check
   phase — the peak-RSS phase — ran. Fixed: dropped at the
   `preliminary_release` boundary after their last reads (eq probe, binding
   merge, final scope build, JSX locator).
2. `ctx.utility_diagnostic_keys` accumulated every checked file's keys on a
   reused parallel worker context (at the time, serial checking cloned a fresh
   context per file, so only parallel runs grew; serial checking has since
   moved to the same single reused context and relies on the same reset).
   Fixed: reset in `begin_file_check` via the baseline/overlay split.
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
