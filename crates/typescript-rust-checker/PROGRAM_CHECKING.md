# Program Checking

`typescript-rust-checker` exposes a program-level entry point for checking multiple files while distinguishing global script files from module files:

- `check_program(files: Vec<SourceFileInput>)`
- `check_program_with_options(files: Vec<SourceFileInput>, options: CheckerOptions)`

The API is intentionally narrow. It now parses a minimal import/export surface and applies a file-level module boundary, but it still stops short of module resolution.

## Public API

The checker crate keeps the existing single-file APIs and adds program-level wrappers:

- `check_source(source_text, file_name)`
- `check_source_with_options(source_text, file_name, options)`
- `check_program(files)`
- `check_program_with_options(files, options)`
- `SourceFileInput`
- `CheckerOptions`

These APIs remain stable in this phase.

## Global Script Model

Program mode treats the input files as one shared global script:

- Top-level `type` aliases are shared across files.
- Top-level `interface` declarations are shared across files.
- Top-level function declarations are shared across files.
- Function bodies can reference shared declarations from earlier or later files.
- Top-level `let`, `const`, and `var` declarations remain file-local.
- Imports and exports are parsed, but module resolution is still unsupported.
- Module files are isolated from the global-script prepass in this phase.
- `declare` and ambient declarations are still unsupported.

## What is shared

- Type declarations and function signatures are collected in a prepass before statement checking begins.
- Declaration diagnostics keep the declaration file name.
- Consuming expression diagnostics keep the consumer file name.
- Script files participate in the global prepass.
- Module files are checked with file-local type declarations and function signatures only.

## Diagnostic Order

Program diagnostics are emitted in a fixed order:

1. Parser diagnostics in input-file order.
2. Global type-declaration diagnostics in input-file order.
3. Global function-signature diagnostics in input-file order.
4. Per-file statement and function-body diagnostics in input-file order.

This ordering is part of the v0.55.1 compatibility contract.

## Type Declarations

- Type aliases and interfaces are stored in one shared namespace.
- The first declaration wins.
- Later duplicates report TS2300 in the duplicate file.
- Duplicate declarations do not replace the original declaration.
- Named-type resolution uses the declaration file while resolving the declaration itself.
- Cycles and unknown names inside a declaration do not cascade into consumer files.

## Function Declarations

- Top-level function declarations are shared across files.
- The first declaration wins for calls.
- Later duplicates report TS2393 in the duplicate file.
- Duplicate function bodies are still checked against their own declared signatures.
- Duplicate function declarations do not replace the global callable symbol.
- Calls use the first declaration's signature.
- Function signature diagnostics are emitted in prepass order, before statement checking.

## Modules

See [MODULES.md](./MODULES.md) for the import/export syntax surface, module-file boundary, and current limitations.

## Local Variables

- Top-level `let`, `const`, and `var` remain file-local in this phase.
- File-local variables do not leak into later files.
- File-local variables are visible to later statements in the same file under the current sequential checker policy.
- File-local variables can be visible inside function bodies when they were declared earlier in the same file.

## Parser Diagnostics

- Parser diagnostics preserve the parser file name.
- Parser errors do not stop type or function prepasses for other files.
- Parser diagnostics do not prevent statement checking of other files.

## CLI Project Mode

`typescript-rust-cli --project` loads all files from the `tsconfig`, checks the program as a whole with `check_program_with_options(...)`, and renders diagnostics grouped by diagnostic file name.

- Diagnostics are grouped in loaded-file order.
- Diagnostics for files not present in the loaded list are rendered at the end when possible.
- `--showConfig` still prints the normalized config and exits successfully.
- Positional single-file mode still uses the single-file checker APIs.

## Upstream Virtual Files

The compatibility harness still splits `// @filename:` comments into virtual files for a small subset of upstream TypeScript-Go fixtures.

- The resulting virtual files are passed to the program checker in `virtual_files` mode.
- This preserves multi-file diagnostic behavior for those fixtures.
- This is still not full upstream baseline compatibility.

## Next phase

The next phase should add the import/export surface and module resolution on top of this program checker rather than reshaping the single-file APIs.
