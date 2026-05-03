# Architecture

This workspace is organized as small crates with stable public façades and internal modules that can evolve without forcing broad API churn.

v0.48 introduced the crate-level module split across types, diagnostics, config, syntax, and checker. v0.48.1 finishes the checker/config/syntax hardening pass by moving the remaining internals into focused submodules while keeping the public crate-root APIs stable.

| Crate | Responsibility |
| --- | --- |
| `typescript-rust-syntax` | Parse TypeScript source into a simplified AST |
| `typescript-rust-types` | Core type representation, display, unions, and assignability |
| `typescript-rust-checker` | Semantic checking and diagnostic emission |
| `typescript-rust-diagnostics` | Diagnostic codes, catalog, constructors, and rendering |
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
- New diagnostics: `typescript-rust-diagnostics` (including CLI-only diagnostics like TS5112)

## Declaration Ingestion
v0.64 adds a foundation for `.d.ts` files, enabling ambient external modules and ambient globals.