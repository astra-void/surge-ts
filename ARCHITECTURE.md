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
- Checker inference is split into expression inference and parsed type resolution.
- Checker symbols are split into value symbols, type declarations, and scope handling.
- The checker now also has a program-level entry point that precollects global
  script declarations across multiple files before statement checking begins.
  It models shared global-script type aliases, interfaces, and top-level
  functions while keeping top-level variables file-local.
- Imports and exports now have a parsed syntax surface, and v0.57.1 hardens a
  focused relative module-resolution-lite layer for loaded program files.
  Module files remain isolated from the global-script sharing prepass, but
  named relative imports, type-only imports, side-effect imports, and local
  named export lists now bind across loaded `.ts` files with separate type and
  value namespaces.
- v0.59 adds a narrow generic syntax surface plus instantiation-lite for type
  aliases and interfaces. v0.59.1 hardens parser recovery, default type
  parameters, arity diagnostics, duplicate type-parameter handling, and
  cross-file generic imports/exports while still keeping generic inference out
  of scope.
- v0.58 adds project compatibility reporting and diagnostic limiting so real
  projects can be triaged without pretending the checker fully supports package
  resolution or the broader TypeScript module surface yet.
- The checker now has a diagnostic span policy document and span-focused
  regression tests; future diagnostics should follow the same span policy
  instead of adding ad-hoc wrapper spans.
- The workspace also has a committed TypeScript oracle comparison harness under
  `scripts/oracle/` that measures diagnostic drift without changing checker
  semantics. It is dev-only tooling and should not pull Node resolution or
  language-service behavior into the Rust crates.
- Future phases should add new modules for interfaces, arrays/tuples, and imports/exports rather than re-expanding monolithic files; literal types are already represented and should be hardened in-place before broader type-system expansion.
- Config, syntax, and checker logic should stay in their dedicated submodule trees rather than returning to crate-root files.
- After v0.59.1, the next phase should be chosen from `--compatReport`
  output, not from a fixed feature wish list.

## Suggested Homes For Future Features

- Interface parsing and checking: `typescript-rust-syntax` and `typescript-rust-checker`
- Arrays and tuples: `typescript-rust-syntax`, `typescript-rust-types`, and `typescript-rust-checker`
- Literal types: `typescript-rust-syntax`, `typescript-rust-types`, and `typescript-rust-checker`
- Imports and exports: `typescript-rust-syntax`, `typescript-rust-checker`, and `typescript-rust-config`
- Program checking: `typescript-rust-checker` and CLI project mode
- Compatibility reporting and triage: `typescript-rust-cli` and `typescript-rust-checker`
- New diagnostics: `typescript-rust-diagnostics`
