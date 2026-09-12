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

## Eligibility for an active case

The pool is `testdata/tests/cases/compiler` at the pinned upstream commit. The
rule for making a case active is a single one: the checker's diagnostic codes
must equal the upstream baseline exactly, in baseline order. A case that drifts
by one code, one file, or one ordering is not added.

For a fixture with an `.errors.txt` baseline, `expected_diagnostics` repeats
that baseline's codes. A fixture with no `.errors.txt` baseline is an upstream
no-error case: it carries an empty `expected_diagnostics` and pins the case as
false-positive free, and its `upstream_baseline_path` points at the `.types`
baseline, which is the reference baseline that does exist for it.

Matching exactly is not the same as being run the way upstream runs it, and the
`reason` field of each case says so where it applies:

- The test-only splitter matches `// @filename:` in lower case with a space. A
  fixture written with `@Filename:` or `//@filename:` is not split, so it is
  checked as a single source.
- `package.json`, `tsconfig.json`, and `node_modules` virtual files are passed
  through as program sources. The harness performs no package resolution and
  reads no tsconfig.
- The fixture's compiler-option header is not applied. Options that gate what
  is checked upstream — `strict`, `allowJs`/`checkJs`, `jsx`, `module`,
  `moduleResolution`, `experimentalDecorators`, `isolatedDeclarations`,
  `exactOptionalPropertyTypes` — have no effect here, and a fixture that
  upstream runs once per setting is run once.

So a green case is a pinned diagnostic-code match against that baseline, not a
claim of upstream baseline compatibility.

## Current limitations

Some upstream TypeScript compiler fixtures use `// @filename:` comments to describe virtual multi-file test cases.

The compatibility test harness still includes a small test-only splitter for these fixtures. In `virtual_files` mode, the split files are passed to the program checker so shared global-script declarations can be checked across file boundaries.

This is useful for early diagnostic-code compatibility, but it is not full upstream baseline compatibility.

The checker now parses import/export syntax and treats files with import/export syntax as module files. Module files remain isolated from the global-script prepass in this phase.

### Historical milestone notes

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
