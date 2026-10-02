# TypeScript-Go Upstream Testdata

This directory tracks a small subset of upstream test cases from:

https://github.com/microsoft/typescript-go/tree/main/testdata

The full upstream testdata suite is intentionally not vendored.

Only small cases are copied here when the current checker can meaningfully run them.

Rules:

- Vendored cases must be copied from the upstream repository unchanged.
- Every vendored case records its upstream path and baseline in
  `manifest.toml` under `[[case]]`; `tests/upstream.test.ts` fails when a file
  under `cases/` has no entry or an entry has no file.
- Custom local tests belong in `tests/smoke` or `tests/checker`, not in this
  directory.
- Do not rewrite upstream tests to fit this checker.

## How cases run

`tests/upstream.test.ts` runs every vendored case through the fixture harness
([tests/README.md](../../README.md)): the case's own `// @filename:` markers
split it into files and its compiler-option directives become the scratch
project's tsconfig, so an upstream `@strict`, `@module`, `@allowJs`, … applies
as written. surge and the pinned tsc 7.0.2 check that project and must report
the same diagnostics. The expectation is tsc run live, not the upstream baseline
file, and a fixture that upstream runs once per option variant is run once with
the first variant.

## Pending records

`[[pending]]` entries are upstream cases that are *not* vendored. Each carries
the upstream baseline's codes (`baseline_diagnostics`) and a `reason` naming
what surge reported. **Those reasons were measured by the Rust smoke harness
retired at `110bbac4` (2026-09-28)**, which ignored compiler-option headers and
did no package resolution; they are a record of the gap at that point, not of
current behaviour. To take a case up, vendor its file, move its entry to
`[[case]]`, and let the live comparison decide.

## Historical milestone notes

**The `v0.5x`/`v0.6x` paragraphs below are historical milestone notes and do
not describe current behavior.** In particular, package resolution,
`node_modules` lookup, `paths`, `baseUrl`, and declaration-file semantics have
since landed on the declaration side; see
[CURRENT_STATUS.md](../../../CURRENT_STATUS.md) and
[crates/surge-ts/MODULE_RESOLUTION.md](../../../crates/surge-ts/MODULE_RESOLUTION.md).
What is still true here is the *scope of this directory*: the upstream fixture
subset is intentionally small and is not a baseline-compatibility claim.

v0.57.1 hardens the limited relative module-resolution-lite pass for loaded program files. v0.61 expands that pass to cover default imports, namespace imports, default exports, named re-exports, type-only re-exports, and star re-exports across already loaded `.ts` files, still with separate type and value namespaces. It still does not implement package resolution, `node_modules`, `paths`, `baseUrl`, star-as re-exports, or other CommonJS/declaration-file semantics. v0.63 adds package import stubbing to reduce cascades from non-relative imports. v0.67 keeps ordinary missing package imports on TS2307 but emits catalog-backed TS2882 for unresolved side-effect imports, matching TypeScript diagnostic priority without adding package lookup.

v0.58 adds compatibility-report instrumentation for real-project triage. External project source should live under `.local-projects/` and should not be committed.

v0.59 adds a narrow generic syntax surface and instantiation-lite for explicit
type arguments on aliases and interfaces. v0.59.1 hardens parser recovery,
defaults, arity diagnostics, and module propagation for those generics while
still keeping constraints parser-only and generic inference out of scope.
The upstream fixture subset here is still intentionally small, and
compatibility-report output should continue to drive the next phase rather than
any expectation of full TypeScript parity.

Note: v0.64 introduced a loaded `.d.ts` ambient subset for globals and exact ambient modules, v0.65 hardens that subset with pinned default-import, namespace-import, re-export, duplicate, and unsupported-syntax behavior, and v0.66 introduced the diagnostic catalog/codegen foundation. These are implemented baselines, not future upstream parity promises. The checker still does not add package discovery, lib.d.ts loading, or @types discovery.

Diagnostics are catalog-driven now, including TS2882, so some compatibility
drift can come from catalog updates even when checker logic stays the same.
Keep those changes intentional and review the generated catalog diff with the
checker diff. Likely next work is diagnostic expansion or package subpath
declaration resolution, not full typescript-go parity.

v0.69 keeps that focus on emitted diagnostics and provides a foundation for package declaration entrypoints. New upstream-style cases should only be added when they map to a real checker or parser emission path with span and no-cascade policy, not just because a code exists in the catalog.
