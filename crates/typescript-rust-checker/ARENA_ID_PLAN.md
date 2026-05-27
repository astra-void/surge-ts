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

- `crates/typescript-rust-checker/Cargo.toml`
- `crates/typescript-rust-checker/src/arena.rs`
- `crates/typescript-rust-checker/src/lib.rs`
- `crates/typescript-rust-checker/src/modules.rs`
- `crates/typescript-rust-checker/src/symbols/scopes.rs`
- `crates/typescript-rust-checker/src/symbols/values.rs`
- `crates/typescript-rust-checker/src/symbols/type_declarations.rs`
- `crates/typescript-rust-checker/src/program.rs`
- `crates/typescript-rust-checker/REAL_PROJECT_COMPAT.md`
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

## What Remains Heap / Value Based

- function-signature payloads
- Direct `SymbolInfo` payload clones outside table/scope snapshot paths are
  still counted separately by `symbol_info_payload_deep_clone_count`.

## Next Slice

No new arena slice starts beyond the v0.99 union-payload handle landing.
This remains a handle-backed slice, not a full type-arena migration.
