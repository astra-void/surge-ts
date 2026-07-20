# Speculative Transactional Checking (STC) — architecture & design

This document is the design reference for parallelizing `surge-ts` semantic
work while keeping `--jobs auto` output byte-identical to the trusted serial
`--jobs 1` path. It consolidates the Stage 0 census
([TRPC-STC-REPORT.md](./TRPC-STC-REPORT.md)) and three subsystem audits
(determinism root-cause, arena/type-store thread-safety, module-graph/SCC
terrain).

`--jobs 1` is the trusted reference path and is never modified for
performance. All parallelism lives beside it and must reproduce its bytes.

## 1. Where the time goes (why this design targets analysis)

tRPC, jobs=1, ~13.4 s internal wall:

| phase | wall | parallel today? |
|---|---|---|
| parsing | 0.28 s | yes |
| ambient + global collection | 0.70 s | no (serial prelude) |
| **preliminary module analysis** | **4.28 s** | **no** |
| **final module analysis** | **3.26 s** | **no** |
| module_binding fixpoint | ~0.16 s stage Δ | no |
| module_local_values | 1.28 s | no |
| **per-file check phase** | **2.99 s** | path exists, gated off for auto |
| finish | 0.45 s | — |

Check-phase parallelism alone floors at ~11 s (measured, jobs=8). **Reaching 5 s
requires parallelizing module analysis (~7.6 s / 56%).** That is the hardest
phase: it *allocates* into bump arenas and *mutates* the declaration/export/
binding tables every later step reads.

## 2. Concurrency substrate (audited)

### 2.1 Arenas (`crates/surge-ts-checker/src/arena.rs`)
`CheckerArena = Arc<CheckerArenaInner>` around a non-thread-safe `oxc` bump
allocator. Allocation asserts `!frozen` (release) and single-owner-thread
(debug). `freeze()` flips an `AtomicBool`; all clones share it. `pending_drops`
runs registered `drop_in_place`s once when the last handle drops.

- **Read concurrently after freeze:** all arena memory (`ArenaStr`, type-decl
  payloads). This is what the check phase already does.
- **Never allocate concurrently into one arena.** The check phase satisfies
  this by freezing before fan-out (`freeze_worker_reachable_arenas`,
  program/mod.rs:1576). **Analysis allocates**, so parallel analysis needs
  **one arena per worker** (owner-thread-satisfying), merged/frozen at join —
  not a shared arena.

### 2.2 Canonical type store (`crates/surge-ts-types/src/store.rs`)
64-way sharded `Mutex` map; `intern_*` take `&self`; monotonic never-reused IDs;
`Weak` payload retention. **Concurrent interning is safe and structurally
deterministic** — the find-or-insert is under one shard lock, so exactly one
`Arc` becomes canonical per key and `Arc::ptr_eq` fast paths stay sound.
**Caveat:** the numeric ID *value* assigned to a logical type is
thread-schedule-dependent. Verified that no diagnostic message, sort key, or
`ObjectType`/type equality derives from a raw store-ID integer (message text,
sort keys use `file_index`; `ObjectType::eq` excludes `property_map_id`). Must
be re-confirmed empirically with 10× determinism runs whenever a new phase
starts interning in parallel.

### 2.3 `CheckerContext` field split (`context.rs`, derived `Clone`)
- **Arc-shared immutable (snapshot):** `options`, `ambient_modules`,
  `module_augmentations`, `ambient_global_type_declarations`,
  `module_file_index_by_identity`, `module_scope_by_file`,
  `module_local_values_by_file`, `file_kinds`, … — safe to share by `Arc`.
- **Shared-mutable, internally synchronized:** `program_type_store` (sharded),
  `substitution_store` + `declaration_environment_store` (single interior
  `Mutex` each — correct, contention-prone).
- **Shared-mutable, single `Mutex` — the risk surface:** the seven caches
  `program_resolved_generic_types`, `program_instantiations`,
  `physical_interface_{instantiations,declaration_templates,method_instantiations,overload_instantiations}`,
  and `timings`. `program_instantiations` is the one that drives the Stage 0
  display divergence.
- **Worker-local (already partitioned):** `file_name`, `diagnostics`, `stats`,
  `symbols`, `type_declarations`, `type_parameter_scopes`, recursion stacks,
  and — reset per file by `begin_file_check` — `resolved_named_types`,
  `type_declaration_scope`, `diagnostic_keys`.

`begin_file_check` (context.rs:1162) makes a *reused* worker context behave
byte-identically to a fresh per-file clone. Workers install the shared store via
the `with_program_type_store` thread-local (store.rs:856) — per-thread handle,
one shared store.

## 3. The determinism rule (non-negotiable)

The only observed jobs=auto output divergence is 7–10 `TS2322` messages where a
user-type array renders nominal `ReadonlyArray<Auth>` (serial) vs structural
`Auth[]` (parallel). Root cause: `program_instantiations` first-writer-wins race
(`lookup_instantiation` cache.rs:997; nominal deferral named.rs:306 vs
structural hit named.rs:238). The divergence is **cosmetic** (`readonly` is not
modeled; both collapse to `Type::Array`), but byte-identity is the gate.

**Forbidden fix:** making the hit render nominal ("normalize the interner-hit
display form") is the change that added 10 diagnostics in the allocation program
— it is semantically visible via union/narrowing. Off the table (CLAUDE.md
boundary).

**Sanctioned fix:** deterministic *publication order* of the shared caches.
Naive per-worker isolation is **not** acceptable — the `SURGE_CHECK_CACHE_ISOLATION`
probe shows isolation flips 28 *other* `TS2339` messages that legitimately
depend on cross-file accumulation. Byte-identity requires reproducing serial's
sequential accumulation visibility.

## 4. Transaction model

```
Immutable Program Snapshot  (Arc, frozen)
        │
        ├── worker: WorkerCheckContext  (snapshot + worker-local scratch)
        │      reads:  snapshot ∪ committed-cache-view ∪ own overlay
        │      writes: own overlay only; records read-set + publications
        ▼
   CheckTransaction { task, file/module id, base_generation,
                      diagnostics, cache_publications, read_set }
        │
        ▼
 Deterministic Coordinator  (single thread)
        commit order: module topo (SCC) → file index → source position
        validate base/dependency generations → publish caches in order →
        merge diagnostics → on conflict: bounded retry, else serial fallback
```

- **Workers never mutate shared semantic state directly.** Cache writes go to a
  worker-local overlay and are returned as `cache_publications`.
- **Coordinator publishes in deterministic order**, so the "first writer" of any
  cache key is always the serial-first file — reproducing serial's display form
  without touching the display logic.
- **Read-set = the cache keys a task looked up** (minimally, whether it hit an
  entry absent from the pre-fan-out committed base). A task whose read-set is
  invalidated by an earlier-ordered commit is re-checked (bounded retry →
  serial fallback). Only the handful of order-sensitive files (≤ ~38 messages
  on tRPC) should ever conflict; the rest commit first-attempt.
- **Provisional IDs never escape a transaction**; the coordinator remaps them at
  commit so canonical `Arc` identity is chosen deterministically, not by worker
  completion order.

## 5. SCC-aware module-analysis scheduling (Stage 5, the 5 s lever)

There is **no dependency graph today** (`import_graph.rs` discards edges; the
analysis loop is flat file-index order, binding.rs:213). Stage 5 must:

1. **Build an import-edge set** (materials exist: `module_file_index_by_identity`
   + specifier scanner/resolver) and run **Tarjan SCC** + condensation.
2. **Schedule by SCC:** independent ready SCCs run in parallel (one worker +
   one arena each); a cyclic SCC runs serially within itself; results commit in
   deterministic condensation-topo → file order.
3. **Hoist the order-sensitive write out of the parallel region:** the final
   pass's first-wins `declare global` value publication
   (`driver.rs:283,324`, gated by `lower_global_augmentation_values`) must be
   pre-scanned/committed serially in a deterministic order, or it will diverge
   exactly like the check-phase race.
4. **Keep the safe serial prelude serial:** ambient/global *interface* merges
   (`ambient.rs`, `driver.rs:131`) are order-insensitive unions — cheap, leave
   them serial before fan-out.
5. **Per-worker arenas** (§2.1): each SCC-worker owns its arena; merge/freeze
   handles at join.

**Open feasibility question — the Amdahl ceiling.** The achievable analysis
speedup is bounded by `largest-SCC-analysis-time / total-analysis-time`, not
node count. tRPC's barrel-heavy `packages/*/src` layout suggests a few large
SCCs around core generic modules (Stage 0 collection-time concentrates in
`utilsProxy.ts`, `createOptionsProxy.ts`, `shared/types.ts`, `createTRPCReact.tsx`).
This must be **measured** (edge dump + Tarjan + per-module analysis time)
before committing to the per-worker-arena refactor. If one SCC dominates, 5 s is
not reachable by SCC parallelism alone and the report must say so honestly.

## 6. Adaptive scheduler (Stage 6)

`--jobs auto` chooses `min(cpu, memory_budget/per_worker, ready_tasks,
graph_width, user_limit).max(1)`. Peak RSS at jobs=8 is only +50 MB over jobs=1
(2.03 vs 1.98 GB), so memory headroom is ample for the check phase; per-worker
*analysis* arenas are the memory variable to watch and to cap workers against.
`--jobs 1` stays fully serial and trusted; `--jobs N` is an explicit upper
bound.

## 7. Mandatory agent rules (for this subsystem)

- MUST NOT mutate shared semantic state from worker threads; return writes via a
  transaction.
- MUST NOT let worker completion order affect diagnostics or canonical `Arc`
  identity; commit deterministically.
- MUST preserve first-wins semantics explicitly (declaration completion, global
  value publication, cache publication).
- MUST bound transaction retries and provide a serial fallback.
- MUST keep `--jobs 1` the trusted reference path.
- MUST NOT alter `type_dedup_fingerprint` hashing or normalize the interner-hit
  display form (both pinned).
- MUST allocate analysis into per-worker arenas; never share one bump arena
  across threads.

## 8. Stage sequence & status

| stage | scope | status |
|---|---|---|
| 0 | baseline, census, feasibility | ✅ committed |
| — | subsystem audits + this design | ✅ committed |
| 5.0 | module-graph edge dump + SCC/Amdahl measurement | next |
| 1–4 | deterministic parallel **check** (transaction + ordered commit) | pending |
| 5 | parallel **module analysis** (SCC + per-worker arenas) | pending, gated on 5.0 |
| 6–7 | adaptive scheduler + contention/memory tuning | pending |
| 8 | full validation matrix + report | pending |

## Deferred resolution (publisher-stamped reservations)

An opt-in extension (`SURGE_DEFER`) adds a publisher-stamped reservation table
alongside the six order-visible caches so a replay can *defer* — and be requeued
— instead of over-recursing on a key an earlier not-yet-committed position will
publish. It is byte-identical and moves the structural conflict metric but does
not reduce tRPC wall time; the sub-file short-circuit that would is the open,
unsafe frontier. See [TRPC-DEFERRED-RESOLUTION.md](TRPC-DEFERRED-RESOLUTION.md).
