# Architecture

`surge-ts` is a Rust-based TypeScript noEmit compatibility checker: it aims for
tsc-compatible diagnostics in noEmit-style project checks. `TypeScript` below
refers to the language/ecosystem being checked, not the project.

This workspace is organized as small crates with stable public façades and internal modules that can evolve without forcing broad API churn.

> Naming note: the public project and reports are named `surge-ts`, and the CLI
> binary command is `surge`. The internal Cargo crates are named `surge-ts-*`
> (e.g. `surge-ts-cli`, `surge-ts-checker`) and crate directories live under
> `crates/surge-ts-*`. Custom diagnostic codes use the `surge::` namespace (e.g.
> `surge::project-has-no-source-files`).

## Current compatibility baseline

Project mode loads the physical `lib*.d.ts` graph from the pinned local
`typescript` package by default. Standard/DOM/global library surfaces and the
utility-type ecosystem come from those loaded `.d.ts` declarations, not from
Rust-side synthetic globals. The generated default-lib subset is only a fallback
used when the `typescript` package cannot be found (and the single-file support
path), not the normal correctness source of truth. `noLib: true` keeps the
standard/DOM globals unavailable. The internal type IR still models primitives
and object/function/union/array/tuple shapes, but those are language-level type
representations rather than ambient library declarations. Node core-module
knowledge, where it exists, is diagnostic/resolution support (e.g. missing-module
hints), not Node global type synthesis.

Measured state: the auth-kit real-project baseline matches TypeScript exactly
(0/0), the oracle preset sweep is green across all registered presets (83 at
commit 6fc9e6c; the count grows as fixtures are added) under the normal gate, and the
`diagnostics-pack` preset is green. The normal gate is diagnostic code-count and
file/code/line; message-text and span/column drift are reported but non-gating
unless `--strictMessages` / `--strictSpans` are passed. Performance notes and
measurement artifacts live in [REAL_PROJECT_COMPAT.md](REAL_PROJECT_COMPAT.md);
this baseline is not a claim of full TypeScript, `lib.d.ts`, DOM, Node, or React
parity.

## Historical version notes

The version-tagged notes throughout this document record how the checker reached
the current state and do not all describe current behavior. In particular, the
pre-physical-lib "synthetic built-ins" (v0.72) and "generated default-lib as the
ambient default" (v0.85) descriptions are historical: physical `lib*.d.ts`
loading is now the default and the generated subset is a fallback.

v0.68.1 hardens diagnostic coverage metadata, ensuring that `support = "emitted"` is backed by test and oracle evidence via an emitted-diagnostics manifest. The `diagnostics-pack` fixture is the compact oracle-backed project for supported emitted diagnostics, now pinned to exact parity in the preset sweep. v0.69 supports narrow bare package declaration entrypoints. v0.69.1 hardens/refactors this support. v0.72/v0.72.1 used synthetic built-ins, not physical `lib.d.ts` (since superseded by physical-lib loading). `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of the mapped-types foundation introduced in v0.80.1. v0.85 added a generated default-lib foundation from the local TypeScript package and loaded those generated declarations as ambient default libs; that path is now the fallback behind physical-lib loading, and `noLib: true` disables both. Full lib.d.ts parity remains future work. v0.82 hardens project/file discovery so project mode cannot silently compare as zero diagnostics when the project surface was never loaded; it also makes `.tsx` visibility explicit without claiming JSX support.

v0.48 introduced the crate-level module split across types, diagnostics, config, syntax, and checker. v0.48.1 finishes the checker/config/syntax hardening pass by moving the remaining internals into focused submodules while keeping the public crate-root APIs stable.

v1.2.5 continues that direction inside `surge-ts-checker` by decomposing the largest checker internals from single files into directory submodules, with no public-API change: `checks/call/` (`mod`, `builtins`, `property`, `instantiate`), `checks/function/` (`mod`, `signature`, `body`, `narrowing`), `infer/expression/` (`mod`, `literals`, `operators`, `access`, `functions`), `infer/types/` (`mod`, `resolve`, `interface`, `utility`, `cache`, `diagnostics`), `modules/` (`mod`, `imports`, `exports`, `resolution`, `diagnostics`), `program/` (`mod`, `binding`, `statements`, `globals`, `ambient`), and `flow/` (`mod`, `branch`, `expr`, `facts`). Counter instrumentation also moved into a dedicated `metrics` module gated behind `--timings`.

| Crate | Responsibility |
| --- | --- |
| `surge-ts-syntax` | Parse TypeScript source into a simplified AST |
| `surge-ts-types` | Core type representation, display, unions, assignability, and the canonical program type store |
| `surge-ts-checker` | Semantic checking and diagnostic emission |
| `surge-ts-diagnostics` | Diagnostic codes, catalog, generated accessors, and rendering |
| `surge-ts-config` | `tsconfig.json` loading, normalization, and file discovery |
| `surge-ts` | Embeddable umbrella crate: `Project` (config load, package/`paths` resolution, default-lib loading, import-graph expansion) plus re-exported checker APIs |
| `surge-ts-cli` | CLI orchestration (built on `surge-ts`) |

## Checking pipeline

Project checking runs the following phases in order (loader phases live in
`crates/surge-ts/src/lib.rs`; checking phases in
`crates/surge-ts-checker/src/program/mod.rs`):

1. **Config load** — `surge-ts-config` loads and normalizes `tsconfig.json`
   (extends chains, include/exclude, compiler options).
2. **File discovery** — the config's include roots are expanded into the
   initial source-file set.
3. **Import-graph expansion** — `Project::check` runs a fixpoint that
   combines `crates/surge-ts/src/import_graph.rs` (relative files,
   `baseUrl`/`paths` mappings) with the package-declaration resolvers
   (`package_declarations.rs` / `package_resolution.rs`: package entrypoints,
   `types`/`typeRoots`, `/// <reference types>` directives) until no new file
   is discovered. The physical `lib*.d.ts` graph from the local `typescript`
   package is loaded here.
4. **Parse** — `parse_program_files` parses all inputs into the simplified
   AST; parsing may fan out to parse workers (one oxc allocator per thread,
   dropped after parsing). Each file is classified by its resolved physical
   path (`classify_file_kind` → `FileKind`: root source/declaration,
   dependency declaration, generated declaration, physical default lib),
   which selects its checking policy (declaration-backed deferral,
   diagnostic suppression, ambient lowering).
5. **Module analysis and binding** — ambient globals/augmentations/ambient
   modules are collected; then a preliminary module-analysis round, a
   multi-round export-table/import-binding/resolution-scope fixpoint, and a
   final module-analysis round produce the shared program state (see
   `crates/surge-ts-checker/PROGRAM_CHECKING.md` for why there are two
   analysis rounds and what may only happen in the final one). Preliminary
   structures are dropped at the `preliminary_release` boundary.
6. **Check** — per-file checking over the shared read-only state, serial or
   parallel (`--jobs`); worker results merge in loaded-file order.
7. **Render** — the CLI groups diagnostics by file in loaded-file order and
   renders via `surge-ts-diagnostics` (`tsc`/`custom`/`json` styles). Report
   tables are explicitly sorted; no output depends on hash-map iteration
   order.

At the end of a run the program caches are torn down
(`clear_program_type_caches` + `ProgramTypeStore::clear`) so a long-lived
embedding process does not retain the run's type graph.

## Canonical type stores

`surge-ts-types` owns a per-run `ProgramTypeStore`
(`crates/surge-ts-types/src/store.rs`) that interns structural type payloads
so identical types are shared instead of re-allocated:

- **Program ownership.** `check_program_with_stats_and_jobs` creates one store
  per run and installs it thread-locally (`with_program_type_store`); each
  check worker installs the same store on its thread. IDs embed a 32-bit
  owner tag, so a `TypeListId`/`FunctionTypeId`/`UnionTypeId`/`PropertyMapId`
  from one program can never be dereferenced against another program's store
  (`belongs_to`). **Type IDs must never cross program owners** — an ID is only
  meaningful together with the store that minted it.
- **Immutable canonical payloads.** Interned payloads
  (`Arc<FunctionTypePayload>`, `Arc<UnionTypePayload>`, `Arc<PropertyMap>`,
  `Arc<[Type]>` parameter lists) are write-once; consumers share them by
  handle. Pointer equality of a shared payload short-circuits structural
  comparison on hot paths.
- **What is canonicalized.** Parameter type lists, function payloads, union
  member lists, and object property maps, plus an overload-merge pair cache.
  Lookups hash a bounded structural fingerprint and then confirm by exact
  structural equality inside the bucket, so a fingerprint collision can never
  return a wrong type.
- **Canonical vs fallback payloads.** Interning is best-effort: values whose
  fingerprint is refused — `Type::Unknown` (the degradation sentinel),
  references that retain resolution context, or over-budget/deep structures —
  fall back to an ordinary uninterned `Arc` payload with no ID. Fallbacks are
  semantically identical, just unshared.
- **Concurrency.** The store is sharded (64 shards per table) behind mutexes
  with a contention counter; it is shared across check workers via `Arc`.
- **Cleanup boundary.** `ProgramTypeStore::clear` at end of run drops every
  interned payload still uniquely owned by the store; the checker-side caches
  that reference them are cleared first (`clear_program_type_caches`).

Per-type details: [FUNCTION_TYPES.md](crates/surge-ts-types/FUNCTION_TYPES.md),
[UNION_TYPES.md](crates/surge-ts-types/UNION_TYPES.md). Memory-region and
lifetime rules: [MEMORY_REGIONS.md](crates/surge-ts-checker/MEMORY_REGIONS.md).
Cross-cutting performance rules:
[docs/PERFORMANCE_INVARIANTS.md](docs/PERFORMANCE_INVARIANTS.md).

## Boundary Rules

- `lib.rs` in each crate should stay façade-like.
- New feature work should land in focused modules, not in crate root files.
- Public crate-root exports should stay stable unless a breaking change is intentional.
- Internal helpers should prefer `pub(crate)` visibility.
- Minimal interfaces are currently implemented as shared type declarations that
  lower to object types in the syntax/checker split; future phases should keep
  that surface small until extends, members, and merging are intentionally added.
- Checker inference is split into expression inference and parsed type resolution or language-service
  behavior into the Rust crates.
- Future phases should add new modules for interfaces, arrays/tuples, and imports/exports rather than re-expanding monolithic files; literal types are already represented and should be hardened in-place before broader type-system expansion.
- Config, syntax, and checker logic should stay in their dedicated submodule trees rather than returning to crate-root files.
- After v0.61, the next phase should still be chosen from `--compatReport`
  output, not from a fixed feature wish list.

## Suggested Homes For Future Features

- Interface parsing and checking: `surge-ts-syntax` and `surge-ts-checker`
- Arrays and tuples: `surge-ts-syntax`, `surge-ts-types`, and `surge-ts-checker`
- Literal types: `surge-ts-syntax`, `surge-ts-types`, and `surge-ts-checker`
- Imports and exports: `surge-ts-syntax`, `surge-ts-checker`, and `surge-ts-config`
- Program checking: `surge-ts-checker` and CLI project mode
- Compatibility reporting and triage: `surge-ts-cli` and `surge-ts-checker`
- Oracle comparison: `scripts/oracle/compare-tsc.ts` for project and file mode
  validation (including --ignoreConfig for standalone file checking) plus diagnostic drift measurement
- New diagnostics: `surge-ts-diagnostics` (catalog-driven, including CLI-only diagnostics like TS5112)
- New project-visibility diagnostics: `surge-ts-cli` may emit a custom `surge::project-has-no-source-files` diagnostic when project discovery returns zero source files
- Checker-local path normalization lives in `surge-ts-checker`; config loading and normalization remain in `surge-ts-config` for tsconfig discovery.
- Default-lib loading lives in `surge-ts-checker` and is shared by single-file and program checking. Project mode loads the physical `lib*.d.ts` graph from the local `typescript` package by default, feeding lib selection from tsconfig into the real ambient declarations; the generated subset is the fallback when that package is absent (and the single-file support path).

## Memory-lifetime model

Retained memory is governed by ownership lifetimes, not by cache pruning. The
canonical type stores in `surge-ts-types` hold `Weak` payload references with
monotonic never-reused IDs; `CheckerArena` registers destructors for every
`Drop`-requiring payload; declaration environments capture compact
stamp-deduplicated table snapshots instead of table copies; qualified-import
payloads are shared across importers while explicitly retaining their owning
arena; and superseded analysis, AST, binding-generation, and TLS state is
released at true-death lifecycle boundaries. The full region model, the
prohibited lifetime shortcuts (expansion-cache pruning, broad re-export
payload sharing, environment-insensitive result sharing), and the measurement
tooling (`SURGE_RETENTION_CENSUS`, `SURGE_PAUSE_AT_STAGE`, `SURGE_RSS`) are
documented in
[crates/surge-ts-checker/MEMORY_REGIONS.md](crates/surge-ts-checker/MEMORY_REGIONS.md)
and [docs/MEMORY-OPTIMIZATION-REPORT.md](docs/MEMORY-OPTIMIZATION-REPORT.md);
the canonical-store retention rules live in
[crates/surge-ts-types/FUNCTION_TYPES.md](crates/surge-ts-types/FUNCTION_TYPES.md).

## Diagnostics

Diagnostics are catalog-driven in `surge-ts-diagnostics`.
The Rust accessors are generated from `diagnostic-messages.json`, and spans remain a checker/parser concern rather than a catalog concern.

The default human-readable CLI output is `tsc`-compatible (`render_diagnostics_tsc`):
`--diagnosticStyle <tsc|custom|json>` selects the renderer and `--pretty` controls
the code-frame form. JSON output is unchanged and still drives the oracle harness,
so this rendering layer never affects diagnostic comparison. See the CLI README for
flag details.

## Declaration Ingestion

v0.65 hardens the v0.64 `.d.ts` foundation so ambient behavior is predictable before any package or lib discovery work lands.

- Loaded `.d.ts` files can contribute ambient globals and exact `declare module "pkg"` blocks.
- Ambient modules resolve before package import stubbing fallback.
- Default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports are pinned for the supported ambient-module subset.
- Duplicate `interface` declarations merge (same file, across global files, reopened `declare module` blocks, and `declare global`); a conflicting property type reports TS2717 with the first declaration winning. Duplicate ambient `var`/`const`/`function` globals stay first-wins / pinned.
- A `declare module "pkg"` block in a module file augments an already-resolved target (merging exported interfaces, adding new exported functions/types); augmenting an unresolved target keeps the TS2307 no-cascade policy.
- Unsupported declaration syntax stays parser-safe and emits a stable pinned diagnostic.
- Current project mode supports focused declaration-side modern package resolution (conditional/pattern `exports`, `imports`, `typesVersions`, self-name), configured `@types`/`typeRoots`, class/static/constructor semantics, physical `lib*.d.ts` loading by default (generated subset as fallback), JSX props checking, and a narrow declaration-merging/module-augmentation slice. Full automatic `@types` discovery, full `lib.d.ts`/Node parity, and full TypeScript parity remain out of scope. `baseUrl` non-relative specifier resolution is supported in the loader (the option is deprecated upstream but honored for compatibility).

## Naming

The public project and reports are named `surge-ts`; the CLI binary command is
`surge`. The internal Cargo crates are `surge-ts-*` and live under
`crates/surge-ts-*`, with `use surge_ts_*` import paths throughout `src/` and
`tests/`. Custom diagnostic codes use the `surge::` namespace (e.g.
`surge::project-has-no-source-files`), and runtime env vars use the `SURGE_`
prefix (`SURGE_PHYSICAL_LIBS`, `SURGE_TIMINGS`).

| Crate | Role |
| --- | --- |
| `surge-ts` | Embeddable umbrella crate (`Project`, loader phases, re-exported checker APIs) |
| `surge-ts-cli` | CLI orchestration (binary `surge`) |
| `surge-ts-checker` | Semantic checking and diagnostic emission |
| `surge-ts-syntax` | Parsing into the simplified AST |
| `surge-ts-types` | Core type representation and canonical type store |
| `surge-ts-diagnostics` | Diagnostic codes, catalog, generated accessors |
| `surge-ts-config` | `tsconfig.json` loading and discovery |
| `surge-ts-diagnostics-codegen` | Catalog code generation |

One internal stats key is intentionally left unchanged: the bench `ts-rust` key
in saved benchmark archive JSON is kept stable so older archives remain
readable.
