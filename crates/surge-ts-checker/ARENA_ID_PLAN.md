# Arena / ID Landing Note

This note records the v0.99 `UnionType` handle landing on top of the v0.98
function-payload handle work, the v0.97 object-payload handle work, and the
v0.96 arena work. `TypeDeclarationTable` still stores arena-owned declaration
payloads behind `TypeDeclarationId` handles while declaration names live in
arena-backed `ArenaStr` keys. `ObjectType` payloads still go through
checker-side shared handles so object clones copy handles instead of
deep-cloning the property map. `FunctionType` payloads do the same for
function signatures, and `UnionType` now does the same for union members.
Module export tables now reuse shared handles for export payloads and borrowed
lookup paths instead of cloning nested symbol/type payloads on read.
Symbol tables and scope visible snapshots now store and copy shared
`SymbolInfo` handles, so `ScopeStack` frame/snapshot maintenance does not
deep-clone nested type payloads.

## Evidence Surface

Reviewers should inspect these files together:

- `crates/surge-ts-checker/Cargo.toml`
- `crates/surge-ts-checker/src/arena.rs`
- `crates/surge-ts-checker/src/lib.rs`
- `crates/surge-ts-checker/src/modules/` (split into `mod.rs`, `imports.rs`, `exports.rs`, `resolution.rs`, `diagnostics.rs`)
- `crates/surge-ts-checker/src/symbols/scopes.rs`
- `crates/surge-ts-checker/src/symbols/values.rs`
- `crates/surge-ts-checker/src/symbols/type_declarations.rs`
- `crates/surge-ts-checker/src/program/` (split into `mod.rs`, `binding.rs`, `statements.rs`, `globals.rs`, `ambient.rs`)
- `REAL_PROJECT_COMPAT.md`
- `.bench/auth-kit-measurement.md`

## v0.96 Evidence

- `checker_arena_alloc_count=21076`
- `arena_declaration_key_alloc_count=10538`
- `arena_type_declaration_payload_alloc_count=10538`
- `type_declaration_table_clone_count=4`
- `type_declaration_id_copy_count=1579`
- `type_declaration_payload_deep_clone_count=15319`
- `type_declaration_entries_merged_total=863`
- `type_clone_count=763`
- `object_type_clone_count=275`
- `union_type_clone_count=98`
- `symbol_name_clone_count=0`
- `string_key_clone_count=0`
- `flow_local_name_clone_count=0`
- `string_path_lookup_count=30470`
- `canonical_file_id_lookup_count=14574`
- `type_name_lookup_string_count=12476`

## v0.97 Evidence

- `checker_arena_alloc_count=23067`
- `arena_object_type_payload_alloc_count=1991`
- `object_type_payload_deep_clone_count=0`
- `object_type_clone_count=275`
- `object_type_id_copy_count=275`
- `type_clone_count=763`
- `union_type_clone_count=98`
- `symbol_name_clone_count=0`
- `string_key_clone_count=0`
- `flow_local_name_clone_count=0`
- `string_path_lookup_count=30470`
- `canonical_file_id_lookup_count=14574`
- `type_name_lookup_string_count=12502`

## v0.97.1 Stabilization

- `contextual-callback-object-properties-basic` oracle match: yes
- `mapped-types-basic` oracle match: yes
- `object_type_payload_deep_clone_count=0`
- `suppressedRustOnlyDiagnosticsTotal=20`
- no UnionType migration started
- FunctionType migration had not started yet

## v0.98 Evidence

- `checker_arena_alloc_count=25491`
- `arena_object_type_payload_alloc_count=1993`
- `object_type_payload_deep_clone_count=0`
- `object_type_clone_count=280`
- `object_type_id_copy_count=280`
- `function_type_payload_alloc_count=2461`
- `function_type_payload_deep_clone_count=0`
- `function_type_handle_copy_count=946542`
- `function_type_clone_count=946542`
- `union_type_clone_count=100`
- `type_clone_count=771`
- `type_name_lookup_string_count=12496`
- `suppressedRustOnlyDiagnosticsTotal=20`
- `FunctionType` handle-backed: yes
- `ObjectType` handle-backed: yes
- `UnionType` value-owned: yes

## v0.98.1 Stabilization

- `contextual-callback-object-properties-basic` oracle match: yes
- `mapped-types-basic` oracle match: yes
- `type-operators-basic` oracle match: yes
- `indexed-access-basic` oracle match: yes
- `function_type_payload_deep_clone_count=0`
- `object_type_payload_deep_clone_count=0`
- `function_type_handle_copy_count=946422`
- `function_type_clone_count=946422`
- `ts-rust jobs=1=0.92s`
- `ts-rust jobs=4=0.89s`
- `jobs=4 no longer regresses versus jobs=1`
- no UnionType migration started
- FunctionType migration was already completed in v0.98 and is preserved here

## v0.99 Evidence

- `checker_arena_alloc_count=25491`
- `arena_object_type_payload_alloc_count=1993`
- `object_type_payload_deep_clone_count=0`
- `object_type_clone_count=280`
- `object_type_id_copy_count=280`
- `function_type_payload_alloc_count=2461`
- `function_type_payload_deep_clone_count=0`
- `function_type_handle_copy_count=946413`
- `function_type_clone_count=946413`
- `union_type_payload_alloc_count=1851`
- `union_type_payload_deep_clone_count=0`
- `union_type_handle_copy_count=10516`
- `union_type_clone_count=100`
- `type_clone_count=771`
- `type_name_lookup_string_count=12502`
- `suppressedRustOnlyDiagnosticsTotal=20`
- `FunctionType` handle-backed: yes
- `ObjectType` handle-backed: yes
- `UnionType` handle-backed: yes
- `TypeDeclarationTable` arena-backed: yes
- no hot allocator mutex

## v1.2.5 Caching / Copy-On-Write Evidence

v1.2.5 is a performance pass that does not add TypeScript semantics and does not
start a new type-IR arena migration. It reduces redundant work around the
existing arena/handle model rather than expanding it. Four changes land:

- Path canonicalization is memoized per run (thread-local, cleared at check
  start) in both `crates/surge-ts-checker/src/paths.rs` and
  `crates/surge-ts-config/src/paths.rs`. Profiling with `/usr/bin/sample`
  showed uncached `std::fs::canonicalize` (`realpath`) as the single largest
  self-time cost; the multi-pass module-binding/resolution fixpoint and the CLI
  import-graph discovery loop canonicalize the same paths repeatedly. This was
  the dominant win.
- Instrumentation counters (`record_program_counter`) are gated behind a
  `COUNTERS_ENABLED` flag set only when `--timings` is requested, so the single
  global `Mutex<ProgramCounters>` is no longer locked on every `SymbolTable::get`
  and table clone in normal runs. Counts stay exact under `--timings`.
- `SymbolTable` is copy-on-write: `symbols: Arc<HashMap<Arc<str>, SymbolInfoHandle>>`,
  `clone` is an `Arc` bump, and `insert`/`insert_handle`/`remove` route through a
  `symbols_mut` helper that `Arc::make_mut`s and records the entry/handle copies
  only when a shared table is actually mutated.
- `resolve_relative_module` is memoized per run via a thread-local
  `(importer, specifier) -> Option<ModuleResolution>` cache, cleared at check
  start because resolved indices are run-specific.

Evidence:

- `symbol_table_clone_count=9143` (unchanged; clones are now `Arc` bumps)
- `symbol_table_entry_handle_copy_count=27698` (was `86782`)
- `symbol_info_handle_copy_count=32988` (was `92072`)
- `ts-rust` auth-kit median: `~0.20s` at `jobs=1`, `~0.19s` at `jobs=4`
  (stable floor near `0.18s`), down from v1.2.4's `0.80s`/`0.78s`
- exact diagnostics `0`, raw oracle match: yes
- `TypeDeclarationTable` arena-backed: yes
- `ObjectType`/`FunctionType`/`UnionType` handle-backed: yes
- no hot allocator mutex; the counter mutex is now gated off the hot path
- The remaining hot cost is the multi-pass binding/resolution recompute itself,
  not allocation. The one untaken arena slice is sharing `TypeDeclarationInfo`
  payloads (`InterfaceInfo`/`TypeAliasInfo`) as handles to avoid deep-cloning
  declarations when they move between tables (see Next Slice).

## v1.2.4 Symbol/Scope Recovery Evidence

v1.2.4 is a performance recovery / stabilization pass after v1.2.3. It does
not add TypeScript semantics and does not start a new arena/type-IR migration.
The v1.2.3 `SymbolInfo` shared-handle model remains in place; the recovery work
reduces whole-table materialization by borrowing visible symbols for local
variable checks, lazily cloning parameter scopes only for parameter
initializers, and restoring `ScopeStack` visible-symbol shadows on pop instead
of rebuilding the flat visible map.

- `function_type_handle_copy_count=2349`
- `union_type_handle_copy_count=1181`
- `function_type_copy_from_scope_or_context_count=1`
- `union_type_copy_from_scope_or_context_count=11`
- `function_type_copy_from_module_export_count=0`
- `union_type_copy_from_module_export_count=0`
- `object_type_payload_deep_clone_count=0`
- `function_type_payload_deep_clone_count=0`
- `union_type_payload_deep_clone_count=0`
- `symbol_info_handle_copy_count=92072`
- `symbol_info_payload_deep_clone_count=6`
- `symbol_table_clone_count=9143`
- `symbol_table_entry_handle_copy_count=86782`
- `scope_stack_visible_rebuild_count=0`
- `scope_stack_visible_symbol_handle_copy_count=513`
- `ts-rust` auth-kit median: `0.80s` at `jobs=1`, `0.78s` at `jobs=4`
- wall-clock improved versus v1.2.3 but remains above v1.2.2
- `TypeDeclarationTable` arena-backed: yes
- `ObjectType` handle-backed: yes
- `FunctionType` handle-backed: yes
- `UnionType` handle-backed: yes
- no hot allocator mutex

## v1.2.3 Symbol/Scope Evidence

- `function_type_handle_copy_count=2400`
- `union_type_handle_copy_count=1175`
- `function_type_copy_from_scope_or_context_count=1`
- `union_type_copy_from_scope_or_context_count=11`
- `function_type_copy_from_module_export_count=0`
- `union_type_copy_from_module_export_count=0`
- `object_type_payload_deep_clone_count=0`
- `function_type_payload_deep_clone_count=0`
- `union_type_payload_deep_clone_count=0`
- `symbol_info_handle_copy_count=118133`
- `symbol_info_payload_deep_clone_count=30`
- `symbol_table_clone_count=10300`
- `symbol_table_entry_handle_copy_count=101938`
- `scope_stack_visible_rebuild_count=239`
- `scope_stack_visible_symbol_handle_copy_count=11474`
- `TypeDeclarationTable` arena-backed: yes
- `ObjectType` handle-backed: yes
- `FunctionType` handle-backed: yes
- `UnionType` handle-backed: yes
- no hot allocator mutex

## Safety Model

- `CheckerArena` is owned per checker run and cloned through `Arc<CheckerArenaInner>`.
- Allocation happens during declaration lowering, before the resulting tables are shared for read-only checking.
- `TypeDeclarationTable` stores declaration payload handles and keeps the arena alive for the cloned tables that reference them.
- Table clone paths copy IDs/handles; they do not clone `TypeDeclarationInfo` payload bodies.
- The arena is never reset while any key or payload handle is still alive.
- `ObjectType` payloads are shared through checker-side handles, so object
  clone paths copy handles instead of deep-cloning property maps.

## What Is Arena-Backed

- Declaration table keys via `ArenaStr`.
- Declaration payloads via arena-backed `TypeDeclarationInfo` handles.

## What Is Handle-Backed

- `ObjectType` property maps and optional string-index payloads.
- `FunctionType` signature payloads.
- `UnionType` member payloads.
- `SymbolInfo` payloads stored in checker `SymbolTable` and `ScopeStack`
  visible snapshots.
- The `SymbolTable` map itself, as of v1.2.5, is `Arc<HashMap<..>>` copy-on-write:
  clones share the map and only deep-copy on the rare mutate-while-shared path.

## What Remains Heap / Value Based

- function-signature payloads
- Direct `SymbolInfo` payload clones outside table/scope snapshot paths are
  still counted separately by `symbol_info_payload_deep_clone_count`.

## Next Slice

No new type-IR arena slice has started beyond the v0.99 union-payload handle
landing. v1.2.5 stayed in the existing model: it added per-run caching
(canonicalization, relative-module resolution), gated the counter mutex, and
made `SymbolTable` copy-on-write, but did not migrate new payloads into the
arena.

The one identified, untaken arena slice is sharing `TypeDeclarationInfo`
payloads as handles. Today `InterfaceInfo`/`TypeAliasInfo` are deep-cloned when
a declaration moves between tables (export-table build, import bindings, and the
fixpoint), which profiling attributes to a few percent of run time. Making the
payloads `Arc<TypeDeclarationInfo>` (or otherwise reference-shared) would turn
those moves into pointer copies, but it touches the unsafe per-table arena
storage and ~13 call sites, so it is deferred until the larger,
correctness-sensitive multi-pass fixpoint reduction is scoped — that recompute,
not allocation, is the dominant remaining cost.
