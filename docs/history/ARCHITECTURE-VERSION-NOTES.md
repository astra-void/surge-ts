# Architecture — historical version notes

**Historical record. Does not describe current behavior.** These notes were
moved out of [ARCHITECTURE.md](../../ARCHITECTURE.md) so the version-tagged
milestone log cannot be mistaken for the current design or support surface.

For what is true now: [CURRENT_STATUS.md](../../CURRENT_STATUS.md). For the
design itself: [ARCHITECTURE.md](../../ARCHITECTURE.md).

The `v0.x` / `v1.x` labels are **internal milestone labels**, not releases,
tags, or crate versions — see
[CURRENT_STATUS.md § Versioning](../../CURRENT_STATUS.md#versioning).

Known corrections, verified 2026-09-01 at commit `37dfb3a`: physical
`lib*.d.ts` loading is the default (the `v0.85` generated default-lib is the
fallback); `baseUrl`, `export =`, `import … = require(…)`, enums, namespaces,
qualified heritage clauses, and automatic `@types` discovery under TS 6/7
semantics are all supported.

---

The version-tagged notes throughout this document record how the checker reached
the current state and do not all describe current behavior. In particular, the
pre-physical-lib "synthetic built-ins" (v0.72) and "generated default-lib as the
ambient default" (v0.85) descriptions are historical: physical `lib*.d.ts`
loading is now the default and the generated subset is a fallback.

v0.68.1 hardens diagnostic coverage metadata, ensuring that `support = "emitted"` is backed by test and oracle evidence via an emitted-diagnostics manifest. The `diagnostics-pack` fixture is the compact oracle-backed project for supported emitted diagnostics, now pinned to exact parity in the preset sweep. v0.69 supports narrow bare package declaration entrypoints. v0.69.1 hardens/refactors this support. v0.72/v0.72.1 used synthetic built-ins, not physical `lib.d.ts` (since superseded by physical-lib loading). `Array<T>` and `ReadonlyArray<T>` are modeled enough to preserve element diagnostics. v0.81 adds narrow synthetic lowering for `Record`, `Partial`, `Pick`, and `Omit` on top of the mapped-types foundation introduced in v0.80.1. v0.85 added a generated default-lib foundation from the local TypeScript package and loaded those generated declarations as ambient default libs; that path is now the fallback behind physical-lib loading, and `noLib: true` disables both. Full lib.d.ts parity remains future work. v0.82 hardens project/file discovery so project mode cannot silently compare as zero diagnostics when the project surface was never loaded; it also makes `.tsx` visibility explicit without claiming JSX support.

v0.48 introduced the crate-level module split across types, diagnostics, config, syntax, and checker. v0.48.1 finishes the checker/config/syntax hardening pass by moving the remaining internals into focused submodules while keeping the public crate-root APIs stable.

v1.2.5 continues that direction inside `surge-ts-checker` by decomposing the largest checker internals from single files into directory submodules, with no public-API change: `checks/call/` (`mod`, `builtins`, `property`, `instantiate`), `checks/function/` (`mod`, `signature`, `body`, `narrowing`), `infer/expression/` (`mod`, `literals`, `operators`, `access`, `functions`), `infer/types/` (`mod`, `resolve`, `interface`, `utility`, `cache`, `diagnostics`), `modules/` (`mod`, `imports`, `exports`, `resolution`, `diagnostics`), `program/` (`mod`, `binding`, `statements`, `globals`, `ambient`), and `flow/` (`mod`, `branch`, `expr`, `facts`). Counter instrumentation also moved into a dedicated `metrics` module gated behind `--timings`.
