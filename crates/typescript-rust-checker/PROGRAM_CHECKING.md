# Program Checking

`typescript-rust-checker` exposes a program-level entry point for checking multiple files while distinguishing global script files from module files:

- `check_program(files: Vec<SourceFileInput>)`
- `check_program_with_options(files: Vec<SourceFileInput>, options: CheckerOptions)`

The API is intentionally narrow. v0.57.1 hardens relative module resolution-lite for loaded program files while keeping the single-file APIs unchanged, v0.59/v0.59.1 add a small generic syntax surface on top of the existing declaration prepass, v0.61 expands the module surface to cover default imports/exports, namespace imports, named re-exports, type-only re-exports, and star re-exports over loaded relative `.ts` files, v0.65 hardens the ambient declaration path for loaded `.d.ts` files, and v0.69/v0.69.1/v0.70 add and harden bare package declaration entrypoint and subpath support. v0.84 hardens already-loaded source and declaration export visibility, including namespace re-exports and exact declaration-only package `types` entrypoints, while staying out of full package/runtime resolution. v0.85 introduces a generated default-lib foundation: it does not load the full official TypeScript lib files at runtime, but instead generates a small supported subset from the local TypeScript package and loads those generated declarations as ambient default libs. `noLib: true` disables the generated default libs. Full lib.d.ts parity remains future work. v0.74 adds a narrow optional chaining and nullish coalescing compatibility foundation for expression evaluation. v0.74.1 hardens nested optional property/call chains, adds optional element access for arrays and tuples, and maintains ?? conservative undefined-removal. v0.82 is the project visibility hardening pass: directory-style `include` roots are treated recursively, `.tsx` files are visible without implying JSX semantics, and project mode now emits an explicit custom diagnostic when it discovers zero source files. v0.83 adds parser-safe binding-pattern parameter diagnostics and arrow-function expression support for `TS7031` on object binding elements without claiming full destructuring or callback contextual typing.

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
surface. None of these semantic checker APIs emit TS5112; TS5112 is strictly a
CLI/config-level diagnostic emitted prior to running semantic checking.

## Global Script Model

Program mode treats the input files as one shared global script:

- Top-level `type` aliases are shared across files.
- Top-level `interface` declarations are shared across files.
- Top-level `typeof value` type queries resolve across files in a narrow type-position subset.
- Constraints are parsed and retained but are not enforced in this phase.
- Top-level function declarations are shared across files.
- Function bodies can reference shared declarations from earlier or later files.
- Top-level `let`, `const`, and `var` declarations remain file-local.
- Relative named imports, type-only named imports, default imports, namespace imports, side-effect imports, local named export lists, named re-exports, type-only re-exports, and star re-exports are resolved across loaded `.ts` files.
- Module files are isolated from the global-script prepass in this phase.
- Loaded `.d.ts` files can contribute the v0.64 ambient subset: simple global type/interface/value/function declarations and exact `declare module "pkg"` blocks.
- Package imports resolve to `.d.ts` entrypoints via `types`, `typings`, `exports["types"]`, or `index.d.ts` and behave like external modules.
- Ambient modules and resolved packages resolve before package import stubbing fallback.
- Default exports, namespace imports, named re-exports, type-only re-exports, and star re-exports inside exact ambient modules and resolved package modules are pinned in this phase.
- Duplicate ambient modules and duplicate ambient globals are first-wins / pinned rather than merged.
- Full package resolution remains unsupported.
- Only declaration-oriented `node_modules` lookup is supported, and declaration files can still act as symbol sources even when `skipLibCheck` suppresses their diagnostics.
- Exact package declaration subpaths are supported; wildcard/runtime subpaths are not.
- Only exact `exports["."].types` / `exports["./x"].types` declaration targets are supported; full exports maps are not.

## What is shared

- Type declarations and function signatures are collected in a prepass before statement checking begins.
- Checker value symbol tables store shared `SymbolInfo` handles. Cloning a
  `SymbolTable` copies handles, not symbol payloads or nested function/union
  type payloads. v1.2.4 keeps that v1.2.3 storage model and reduces hot
  materialization by borrowing visible symbols for function-local variable
  checks, lazily cloning function-parameter scope tables only when parameter
  initializers need them, and restoring `ScopeStack` visible-symbol shadows on
  pop instead of rebuilding the whole flat visible map. v1.2.5 makes the
  `SymbolTable` map itself copy-on-write (`Arc<HashMap<..>>` with `Arc::make_mut`
  on mutation), so the multi-pass module-binding fixpoint's table clones share
  one map and only deep-copy when a shared table is actually mutated.
- v1.2.5 also adds per-run memoization for path canonicalization and relative
  module resolution, and gates the instrumentation-counter mutex behind
  `--timings`. These caches are thread-local and cleared at the start of each
  `check_program` run, so they never leak resolved indices or canonical paths
  across runs. They are pure performance changes with no effect on emitted
  diagnostics.
- `SymbolInfo` entries are treated as immutable once shared. Narrowing or
  assignment-style updates create a replacement `SymbolInfo` and swap the table
  handle instead of mutating a shared payload in place.
- Declaration diagnostics keep the declaration file name.
- Consuming expression diagnostics keep the consumer file name.
- Script files participate in the global prepass.
- Module files are checked with file-local type declarations and function signatures plus resolved module bindings.
- Generated default libs are injected ahead of program files when they are not already present in the input set, so direct program-checker callers get the same ambient core/DOM subset as CLI project mode.
- Loaded declaration files are checked as ambient inputs, not as normal runtime files.
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
- Unannotated object binding elements in function and arrow parameters now emit TS7031 on the local binding name span when `noImplicitAny` is enabled.
- Arrow function expressions are parsed and checked as expression values so their parameter diagnostics still fire in variable initializers, call arguments, and expression statements.
- This phase still does not implement full destructuring, callback contextual typing, or generic callback inference.

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
- `--jobs` is deterministic project-checking infrastructure only. Shared loading, graph construction, declaration collection, and module binding remain serial; only the per-file checking phase can run in parallel. Worker results are merged by loaded-file order, not completion order.
- `--showConfig` still prints the normalized config and exits successfully.
- `--showSpans` prints the diagnostic code and span metadata before the rendered excerpt.
- `--compatReport` prints a compatibility summary with loaded-file count,
  total diagnostic count, counts by code, counts by file, parser-error
  grouping, file-kind counts, suppressed totals, and a visibility warning
  when no source files were loaded. It is raw measurement, not a root-cause
  classifier.
- Generic type arguments on references are parsed and lowered for explicit
  alias/interface instantiation.
- v1.1 supports narrow generic indexed access after concrete substitution,
  including `T["key"]`, `T[K]`, and `T[keyof T]` when the receiver/key have
  been substituted to concrete types. Fully unresolved generic indexed access
  and constraint enforcement remain unsupported.
- Narrow generic call-site inference exists for simple direct calls, repeated-
  parameter calls, and array-element calls. Explicit type arguments are still
  preserved and applied when present, but full generic inference, overload
  resolution, generic classes, callback contextual typing, higher-order
  inference, and tuple-valued implicit generic returns remain unsupported.
- `--maxDiagnostics` limits rendered diagnostics but does not change the total
  counts in the compatibility summary.
- `.tsx` files have a parser-safe JSX foundation: JSX elements, fragments,
  attributes, and `{...}` expression containers parse in expression position, and
  JSX expressions infer a conservative `JSX.Element` stand-in (an opaque empty
  object that renders as `Element`). Child and attribute expression containers
  and capitalized component / member tags are walked for ordinary diagnostics
  (so unresolved names still report `TS2304`). `JSX` namespace resolution,
  `JSX.IntrinsicElements` prop validation, React globals, DOM globals, and the
  JSX transform remain out of scope.
- Positional single-file mode still uses the single-file checker APIs and does not resolve sibling files.
- Diagnostic span policy lives in [DIAGNOSTIC_SPANS.md](./DIAGNOSTIC_SPANS.md).
- v0.81 synthetic utility lowering covers `Record<K, T>`, `Partial<T>`, `Pick<T, K>`, and `Omit<T, K>` for concrete object/interface shapes and string-literal key unions.
- A narrow conditional-type evaluator handles `T extends U ? X : Y` (concrete evaluation plus distribution over a naked type parameter), which backs `Exclude`, `Extract`, and `NonNullable`; `ReturnType`/`Parameters` stay as synthetic lowerings over concrete function types. This does not imply physical `lib.d.ts`, `@types`, DOM/Node globals, conditional-type inference, nested/arbitrary `infer`, recursive conditionals, or the rest of the utility-type ecosystem; unsupported conditionals degrade to `unknown`, and full index signatures remain unsupported.
- A narrow template literal type evaluator expands a template into a deduped string-literal union when every interpolation resolves to a finite string/number/boolean literal union (including after generic substitution and over `keyof`). Broad or unresolved interpolations (e.g. `` `id:${string}` ``) degrade to `string` rather than cascading, which under-reports relative to tsc but never yields a false positive. Recursive expansion, in-template `infer`/pattern matching, and intrinsic string utilities (`Uppercase`, etc.) remain unsupported.

## Upstream Virtual Files

The compatibility harness still splits `// @filename:` comments into virtual files for a small subset of upstream TypeScript-Go fixtures.

- The resulting virtual files are passed to the program checker in `virtual_files` mode.
- This preserves multi-file diagnostic behavior for those fixtures.
- This is still not full upstream baseline compatibility.

## Next phase

The next phase should still be chosen from compatibility-report output rather
than reshaping the single-file APIs.

## Ambient Globals

Ambient globals from loaded `.d.ts` files are gathered into `ambient_global_symbols` and `ambient_global_type_declarations`, then mixed into modules and scripts.

## Diagnostic Profiles

The checker supports two diagnostic profiles, controlled via `CheckerOptions.diagnostic_profile` or the `--diagnosticProfile` CLI flag:

1. **`Tsc` (Default)**: Aims to match the TypeScript compiler (`tsc`) exactly. This includes applying targeted type widening (like on `satisfies`) and allowing known diagnostic cascades (like outer assignability failures) to appear if TypeScript emits them.
2. **`Native`**: Uses `typescript-rust`-specific behaviors. For example, it aggressively returns `Unknown` from failed contextual checks (like `satisfies` failures) to suppress noisy downstream cascade errors. This produces a cleaner developer experience but diverges from the TypeScript compiler baseline.

The `compat-projects` oracle testing runs exclusively in `tsc` profile.
`CheckerOptions::default().diagnostic_profile` is `DiagnosticProfile::Tsc`, and the CLI default stays `tsc` unless `--diagnosticProfile native` is explicitly requested.
