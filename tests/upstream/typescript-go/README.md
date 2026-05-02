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

The checker now parses a minimal import/export surface and treats files with import/export syntax as module files, but it still does not resolve modules or imports. Module files remain isolated from the global-script prepass in this phase.
