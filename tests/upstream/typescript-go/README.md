# TypeScript-Go Upstream Testdata

This directory tracks a small subset of upstream test cases from:

https://github.com/microsoft/typescript-go/tree/main/testdata

The full upstream testdata suite is intentionally not vendored.

Only small cases are copied here when the current checker can meaningfully run them.

Rules:

- Active upstream cases must be copied from the upstream repository.
- Active cases must include the original upstream path in `manifest.toml`.
- Pending cases are tracked but not executed.
- Custom local tests belong in `tests/smoke`, not in this directory.
- Do not rewrite upstream tests to fit this checker.

## Current limitations

Some upstream TypeScript compiler fixtures use `// @filename:` comments to describe virtual multi-file test cases.

The compatibility test harness still includes a small test-only splitter for these fixtures. In `virtual_files` mode, the split files are passed to the program checker so shared global-script declarations can be checked across file boundaries.

This is useful for early diagnostic-code compatibility, but it is not full upstream baseline compatibility.

The checker now parses import/export syntax and treats files with import/export syntax as module files. Module files remain isolated from the global-script prepass in this phase.

v0.57.1 hardens the limited relative module-resolution-lite pass for loaded program files. v0.61 expands that pass to cover default imports, namespace imports, default exports, named re-exports, type-only re-exports, and star re-exports across already loaded `.ts` files, still with separate type and value namespaces. It still does not implement package resolution, `node_modules`, `paths`, `baseUrl`, star-as re-exports, or other CommonJS/declaration-file semantics. v0.63 adds package import stubbing to reduce cascades from non-relative imports.

v0.58 adds compatibility-report instrumentation for real-project triage. External project source should live under `.local-projects/` and should not be committed.

v0.59 adds a narrow generic syntax surface and instantiation-lite for explicit
type arguments on aliases and interfaces. v0.59.1 hardens parser recovery,
defaults, arity diagnostics, and module propagation for those generics while
still keeping constraints parser-only and generic inference out of scope.
The upstream fixture subset here is still intentionally small, and
compatibility-report output should continue to drive the next phase rather than
any expectation of full TypeScript parity.

Note: v0.64 introduced ambient declaration files `.d.ts` but does not aim for full typescript-go parity yet.