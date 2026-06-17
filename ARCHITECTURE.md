# Architecture

This workspace is organized as small crates with stable public façades and internal modules that can evolve without forcing broad API churn.

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
(0/0), the oracle preset sweep is 75/75 under the normal gate, and the
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

v1.2.5 continues that direction inside `typescript-rust-checker` by decomposing the largest checker internals from single files into directory submodules, with no public-API change: `checks/call/` (`mod`, `builtins`, `property`, `instantiate`), `checks/function/` (`mod`, `signature`, `body`, `narrowing`), `infer/expression/` (`mod`, `literals`, `operators`, `access`, `functions`), `infer/types/` (`mod`, `resolve`, `interface`, `utility`, `cache`, `diagnostics`), `modules/` (`mod`, `imports`, `exports`, `resolution`, `diagnostics`), `program/` (`mod`, `binding`, `statements`, `globals`, `ambient`), and `flow/` (`mod`, `branch`, `expr`, `facts`). Counter instrumentation also moved into a dedicated `metrics` module gated behind `--timings`.

| Crate | Responsibility |
| --- | --- |
| `typescript-rust-syntax` | Parse TypeScript source into a simplified AST |
| `typescript-rust-types` | Core type representation, display, unions, and assignability |
| `typescript-rust-checker` | Semantic checking and diagnostic emission |
| `typescript-rust-diagnostics` | Diagnostic codes, catalog, generated accessors, and rendering |
| `typescript-rust-config` | `tsconfig.json` loading, normalization, and file discovery |
| `typescript-rust-cli` | CLI orchestration |

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

- Interface parsing and checking: `typescript-rust-syntax` and `typescript-rust-checker`
- Arrays and tuples: `typescript-rust-syntax`, `typescript-rust-types`, and `typescript-rust-checker`
- Literal types: `typescript-rust-syntax`, `typescript-rust-types`, and `typescript-rust-checker`
- Imports and exports: `typescript-rust-syntax`, `typescript-rust-checker`, and `typescript-rust-config`
- Program checking: `typescript-rust-checker` and CLI project mode
- Compatibility reporting and triage: `typescript-rust-cli` and `typescript-rust-checker`
- Oracle comparison: `scripts/oracle/compare-tsc.ts` for project and file mode
  validation (including --ignoreConfig for standalone file checking) plus diagnostic drift measurement
- New diagnostics: `typescript-rust-diagnostics` (catalog-driven, including CLI-only diagnostics like TS5112)
- New project-visibility diagnostics: `typescript-rust-cli` may emit a custom `typescript-rust::project-has-no-source-files` diagnostic when project discovery returns zero source files
- Checker-local path normalization lives in `typescript-rust-checker`; config loading and normalization remain in `typescript-rust-config` for tsconfig discovery.
- Default-lib loading lives in `typescript-rust-checker` and is shared by single-file and program checking. Project mode loads the physical `lib*.d.ts` graph from the local `typescript` package by default, feeding lib selection from tsconfig into the real ambient declarations; the generated subset is the fallback when that package is absent (and the single-file support path).

## Diagnostics

Diagnostics are catalog-driven in `typescript-rust-diagnostics`.
The Rust accessors are generated from `diagnostic-messages.json`, and spans remain a checker/parser concern rather than a catalog concern.

## Declaration Ingestion

v0.65 hardens the v0.64 `.d.ts` foundation so ambient behavior is predictable before any package or lib discovery work lands.

- Loaded `.d.ts` files can contribute ambient globals and exact `declare module "pkg"` blocks.
- Ambient modules resolve before package import stubbing fallback.
- Default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports are pinned for the supported ambient-module subset.
- Duplicate `interface` declarations merge (same file, across global files, reopened `declare module` blocks, and `declare global`); a conflicting property type reports TS2717 with the first declaration winning. Duplicate ambient `var`/`const`/`function` globals stay first-wins / pinned.
- A `declare module "pkg"` block in a module file augments an already-resolved target (merging exported interfaces, adding new exported functions/types); augmenting an unresolved target keeps the TS2307 no-cascade policy.
- Unsupported declaration syntax stays parser-safe and emits a stable pinned diagnostic.
- Current project mode supports focused declaration-side modern package resolution (conditional/pattern `exports`, `imports`, `typesVersions`, self-name), configured `@types`/`typeRoots`, class/static/constructor semantics, physical `lib*.d.ts` loading by default (generated subset as fallback), JSX props checking, and a narrow declaration-merging/module-augmentation slice. Full automatic `@types` discovery, full `lib.d.ts`/Node parity, and full TypeScript parity remain out of scope. `baseUrl` remains unsupported/deprecated.
