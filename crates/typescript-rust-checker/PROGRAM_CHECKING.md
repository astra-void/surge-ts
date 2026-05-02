# Program Checking

`typescript-rust-checker` exposes a program-level entry point for checking multiple files while distinguishing global script files from module files:

- `check_program(files: Vec<SourceFileInput>)`
- `check_program_with_options(files: Vec<SourceFileInput>, options: CheckerOptions)`

The API is intentionally narrow. v0.57.1 hardens relative module resolution-lite for loaded program files while keeping the single-file APIs unchanged, v0.59/v0.59.1 add a small generic syntax surface on top of the existing declaration prepass, and v0.61 expands the module surface to cover default imports/exports, namespace imports, named re-exports, type-only re-exports, and star re-exports over loaded relative `.ts` files.

## Public API

The checker crate keeps the existing single-file APIs and adds program-level wrappers:

- `check_source(source_text, file_name)`
- `check_source_with_options(source_text, file_name, options)`
- `check_program(files)`
- `check_program_with_options(files, options)`
- `SourceFileInput`
- `CheckerOptions`

These APIs remain stable in this phase. v0.58 adds compatibility-report
instrumentation in the CLI on top of these APIs without changing the checker
surface.

## Global Script Model

Program mode treats the input files as one shared global script:

- Top-level `type` aliases are shared across files.
- Top-level `interface` declarations are shared across files.
- Top-level generic aliases and interfaces are shared across files, with explicit type arguments substituted during lowering.
- Defaults on generic aliases and interfaces are applied when explicit type arguments are omitted.
- Constraints are parsed and retained but are not enforced in this phase.
- Top-level function declarations are shared across files.
- Function bodies can reference shared declarations from earlier or later files.
- Top-level `let`, `const`, and `var` declarations remain file-local.
- Relative named imports, type-only named imports, default imports, namespace imports, side-effect imports, local named export lists, named re-exports, type-only re-exports, and star re-exports are resolved across loaded `.ts` files.
- Module files are isolated from the global-script prepass in this phase.
- `declare` and ambient declarations are still unsupported.

## What is shared

- Type declarations and function signatures are collected in a prepass before statement checking begins.
- Declaration diagnostics keep the declaration file name.
- Consuming expression diagnostics keep the consumer file name.
- Script files participate in the global prepass.
- Module files are checked with file-local type declarations and function signatures plus resolved module bindings.
- Single-file checking still does not read sibling files.

## Diagnostic Order

Program diagnostics are emitted in a fixed order:

1. Parser diagnostics in input-file order.
2. Global type-declaration diagnostics in input-file order.
3. Global function-signature diagnostics in input-file order.
4. Module export/import diagnostics in input-file order, including default-import/default-export checks, namespace imports, named re-exports, type-only re-exports, and star re-exports.
5. Per-file statement and function-body diagnostics in input-file order.

This ordering is part of the v0.55.1 compatibility contract.

## Type Declarations

- Type aliases and interfaces are stored in one shared namespace.
- Generic aliases and interfaces still live in that same namespace; arity errors are diagnosed on the type reference span and type-parameter references inside the declaration lower to the instantiated body.
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
- `--showSpans` prints the diagnostic code and span metadata before the rendered excerpt.
- `--compatReport` prints a compatibility summary with loaded-file count,
  total diagnostic count, counts by code, counts by file, and parser-error
  grouping.
- Generic type arguments on references are parsed and lowered for explicit
  alias/interface instantiation, but generic inference is still intentionally
  omitted.
- Call-site type arguments are parsed and preserved for syntax stability, but the checker currently ignores them.
- `--maxDiagnostics` limits rendered diagnostics but does not change the total
  counts in the compatibility summary.
- Positional single-file mode still uses the single-file checker APIs and does not resolve sibling files.
- Diagnostic span policy lives in [DIAGNOSTIC_SPANS.md](./DIAGNOSTIC_SPANS.md).

## Upstream Virtual Files

The compatibility harness still splits `// @filename:` comments into virtual files for a small subset of upstream TypeScript-Go fixtures.

- The resulting virtual files are passed to the program checker in `virtual_files` mode.
- This preserves multi-file diagnostic behavior for those fixtures.
- This is still not full upstream baseline compatibility.

## Next phase

The next phase should still be chosen from compatibility-report output rather
than reshaping the single-file APIs.
